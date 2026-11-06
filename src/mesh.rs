use crate::protocol::{MeshPacket, CONTROL_PORT, MAX_NODES};
use get_if_addrs::{get_if_addrs, IfAddr};
use rand::random;
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{atomic::{AtomicBool, Ordering}, mpsc::{self, Receiver, Sender}, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const IDLE_TIMEOUT: Duration = Duration::from_secs(9);
const HELLO_INTERVAL: Duration = Duration::from_secs(2);
const POLL_TIME: Duration = Duration::from_millis(120);
const MAX_CONTROL_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerView {
    pub id: u64,
    pub name: String,
    pub address: SocketAddr,
    pub voice: bool,
}

#[derive(Debug)]
pub enum MeshEvent {
    Peers(Vec<PeerView>),
    Chat { name: String, text: String },
    Notice(String),
    LearnedAddress(SocketAddr),
}

enum Command {
    Chat(String),
    AddPeer(SocketAddr),
    Voice(bool),
    Rename(String),
    Stop,
}

#[derive(Debug)]
struct ActivePeer {
    view: PeerView,
    last_seen: Instant,
}

struct PendingChat {
    to: SocketAddr,
    bytes: Vec<u8>,
    last_sent: Instant,
    retries: u8,
}

pub struct Mesh {
    pub id: u64,
    pub local_radmin_ips: Vec<Ipv4Addr>,
    commands: Sender<Command>,
    pub events: Receiver<MeshEvent>,
    pub voice_targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    stopped: Arc<AtomicBool>,
}

impl Drop for Mesh {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = self.commands.send(Command::Stop);
    }
}

fn is_valid_address(address: SocketAddr) -> bool {
    match address.ip() {
        IpAddr::V4(ip) => ip.octets()[0] == 26 || ip.is_loopback(),
        IpAddr::V6(_) => false,
    }
}

pub fn radmin_interfaces() -> (Vec<Ipv4Addr>, Vec<SocketAddr>) {
    let mut addresses = Vec::new();
    let mut broadcasts = Vec::new();
    if let Ok(interfaces) = get_if_addrs() {
        for interface in interfaces {
            if let IfAddr::V4(v4) = interface.addr {
                // Radmin VPN assigns virtual IPv4 addresses in 26.0.0.0/8.
                if v4.ip.octets()[0] != 26 { continue; }
                addresses.push(v4.ip);
                let broadcast = v4.broadcast.unwrap_or_else(|| {
                    let ip = u32::from(v4.ip);
                    let mask = u32::from(v4.netmask);
                    Ipv4Addr::from(ip | !mask)
                });
                broadcasts.push(SocketAddr::V4(SocketAddrV4::new(broadcast, CONTROL_PORT)));
            }
        }
    }
    addresses.sort_unstable();
    addresses.dedup();
    broadcasts.sort_unstable();
    broadcasts.dedup();
    (addresses, broadcasts)
}

fn serialize(packet: &MeshPacket) -> Option<Vec<u8>> {
    let bytes = serde_json::to_vec(packet).ok()?;
    (bytes.len() <= MAX_CONTROL_BYTES).then_some(bytes)
}

fn send(socket: &UdpSocket, packet: &MeshPacket, address: SocketAddr) {
    if !is_valid_address(address) { return; }
    if let Some(bytes) = serialize(packet) {
        let _ = socket.send_to(&bytes, address);
    }
}

fn refresh(
    peers: &HashMap<SocketAddr, ActivePeer>,
    events: &Sender<MeshEvent>,
    voice_targets: &Arc<Mutex<Vec<Ipv4Addr>>>,
) {
    let mut list = peers.values().map(|p| p.view.clone()).collect::<Vec<_>>();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    let mut ips = list.iter().filter_map(|p| match p.address.ip() {
        IpAddr::V4(ip) => Some(ip),
        _ => None,
    }).collect::<Vec<_>>();
    ips.sort_unstable();
    ips.dedup();
    if let Ok(mut shared) = voice_targets.lock() { *shared = ips; }
    let _ = events.send(MeshEvent::Peers(list));
}

impl Mesh {
    pub fn start(name: String, saved: Vec<SocketAddr>) -> Result<Self, String> {
        Self::start_on(name, saved, CONTROL_PORT, true)
    }

    /// Custom ports make it possible to test multiple independent nodes on one PC.
    pub fn start_on(name: String, saved: Vec<SocketAddr>, port: u16, broadcast: bool) -> Result<Self, String> {
        let socket = UdpSocket::bind(("0.0.0.0", port))
            .map_err(|e| format!("Не вдалося відкрити UDP-порт {port}: {e}"))?;
        socket.set_broadcast(true).map_err(|e| e.to_string())?;
        socket.set_read_timeout(Some(POLL_TIME)).map_err(|e| e.to_string())?;
        let (ips, mut broadcast_targets) = radmin_interfaces();
        if !broadcast { broadcast_targets.clear(); }
        let id = random::<u64>().max(1);
        let (tx, rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let targets = Arc::new(Mutex::new(Vec::new()));
        let targets_thread = Arc::clone(&targets);
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stopped);
        thread::spawn(move || {
            let mut peers: HashMap<SocketAddr, ActivePeer> = HashMap::new();
            let mut seeds: HashSet<SocketAddr> = saved.into_iter()
                .filter(|a| is_valid_address(*a) && a.port() != 0).collect();
            let mut nickname = name;
            let mut in_voice = false;
            let mut last_hello = Instant::now() - HELLO_INTERVAL;
            let mut next_seq = 1u64;
            let mut pending: HashMap<(SocketAddr, u64), PendingChat> = HashMap::new();
            let mut chat_seen: HashSet<(u64,u64)> = HashSet::new();
            let mut buf = [0u8; MAX_CONTROL_BYTES + 1];
            while !worker_stop.load(Ordering::Relaxed) {
                // Process all UI commands without blocking the UDP receiver.
                let mut flush_hello = false;
                while let Ok(command) = rx.try_recv() {
                    match command {
                        Command::Stop => { worker_stop.store(true, Ordering::Relaxed); break; }
                        Command::Rename(new_name) => { nickname = new_name; flush_hello = true; }
                        Command::Voice(joined) => { in_voice = joined; flush_hello = true; }
                        Command::AddPeer(address) => {
                            if is_valid_address(address) && address.port() != 0 {
                                seeds.insert(address);
                                send(&socket, &MeshPacket::Hello { id, name: nickname.clone(), voice: in_voice }, address);
                                let _ = event_tx.send(MeshEvent::Notice(format!("Запрошення надіслано: {}", address.ip())));
                            }
                        }
                        Command::Chat(text) => {
                            let text = text.trim().to_string();
                            if text.is_empty() || text.chars().count() > 1000 { continue; }
                            let packet = MeshPacket::Chat { id, seq: next_seq, name: nickname.clone(), text: text.clone() };
                            if let Some(bytes) = serialize(&packet) {
                                for address in peers.keys().copied() {
                                    let _ = socket.send_to(&bytes, address);
                                    pending.insert((address, next_seq), PendingChat {
                                        to: address, bytes: bytes.clone(), last_sent: Instant::now(), retries: 0,
                                    });
                                }
                            }
                            let _ = event_tx.send(MeshEvent::Chat { name: nickname.clone(), text });
                            next_seq = next_seq.wrapping_add(1);
                        }
                    }
                }
                if flush_hello {
                    last_hello = Instant::now() - HELLO_INTERVAL;
                }
                if last_hello.elapsed() >= HELLO_INTERVAL {
                    let hello = MeshPacket::Hello { id, name: nickname.clone(), voice: in_voice };
                    let destinations = broadcast_targets.iter().copied()
                        .chain(seeds.iter().copied())
                        .chain(peers.keys().copied()).collect::<HashSet<_>>();
                    for address in destinations {
                        send(&socket, &hello, address);
                    }
                    last_hello = Instant::now();
                }
                // A missing UDP broadcast is not fatal: saved contacts and gossip also discover nodes.
                match socket.recv_from(&mut buf) {
                    Ok((n, source)) if n <= MAX_CONTROL_BYTES && is_valid_address(source) => {
                        let Ok(packet) = serde_json::from_slice::<MeshPacket>(&buf[..n]) else { continue; };
                        match packet {
                            MeshPacket::Hello { id: remote_id, name, voice } => {
                                if remote_id == id || name.trim().is_empty() || name.chars().count() > 24 {
                                    continue;
                                }
                                let now = Instant::now();
                                let new = !peers.contains_key(&source);
                                if new && peers.len() >= MAX_NODES - 1 { continue; }
                                let changed = peers.get(&source)
                                    .map(|peer| peer.view.id != remote_id || peer.view.name != name || peer.view.voice != voice)
                                    .unwrap_or(true);
                                peers.insert(source, ActivePeer {
                                    view: PeerView { id: remote_id, name: name.clone(), address: source, voice },
                                    last_seen: now,
                                });
                                if seeds.insert(source) {
                                    let _ = event_tx.send(MeshEvent::LearnedAddress(source));
                                }
                                if new {
                                    send(&socket, &MeshPacket::Hello {
                                        id, name: nickname.clone(), voice: in_voice,
                                    }, source);
                                    let _ = event_tx.send(MeshEvent::Notice(format!("{name} у мережі")));
                                }
                                // Send known addresses so peers can form a full mesh independently.
                                if new {
                                    let addresses = peers.keys().copied().filter(|a| *a != source).take(4).collect();
                                    send(&socket, &MeshPacket::Peers { id, addresses }, source);
                                }
                                if changed { refresh(&peers, &event_tx, &targets_thread); }
                            }
                            MeshPacket::Peers { id: remote_id, addresses } => {
                                if !peers.get(&source).is_some_and(|p| p.view.id == remote_id) { continue; }
                                for address in addresses.into_iter().take(MAX_NODES) {
                                    if !is_valid_address(address) || address == source || address.port() == 0 { continue; }
                                    if seeds.insert(address) {
                                        send(&socket, &MeshPacket::Hello {
                                            id, name: nickname.clone(), voice: in_voice,
                                        }, address);
                                    }
                                }
                            }
                            MeshPacket::Goodbye { id: remote_id } => {
                                if peers.get(&source).is_some_and(|p| p.view.id == remote_id) {
                                    if let Some(old) = peers.remove(&source) {
                                        let _ = event_tx.send(MeshEvent::Notice(format!("{} вийшов(-ла)", old.view.name)));
                                    }
                                    refresh(&peers, &event_tx, &targets_thread);
                                }
                            }
                            MeshPacket::Chat { id: remote_id, seq, name: _, text } => {
                                if text.chars().count() > 1000 { continue; }
                                let Some(peer) = peers.get_mut(&source) else { continue; };
                                if peer.view.id != remote_id { continue; }
                                peer.last_seen = Instant::now();
                                send(&socket, &MeshPacket::Ack { id, seq }, source);
                                if chat_seen.insert((remote_id, seq)) {
                                    if chat_seen.len() > 4096 { chat_seen.clear(); }
                                    let _ = event_tx.send(MeshEvent::Chat {
                                        name: peer.view.name.clone(), text,
                                    });
                                }
                            }
                            MeshPacket::Ack { id: remote_id, seq } => {
                                if peers.get(&source).is_some_and(|p| p.view.id == remote_id) {
                                    pending.remove(&(source, seq));
                                }
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock ||
                        error.kind() == std::io::ErrorKind::TimedOut ||
                        error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => {
                        let _ = event_tx.send(MeshEvent::Notice(format!("UDP: {error}")));
                        thread::sleep(Duration::from_millis(250));
                    }
                }
                let now = Instant::now();
                let old_len = peers.len();
                peers.retain(|_,peer| now.duration_since(peer.last_seen) < IDLE_TIMEOUT);
                if peers.len() != old_len {
                    refresh(&peers, &event_tx, &targets_thread);
                }
                pending.retain(|(address, _), delivery| {
                    if !peers.contains_key(address) || delivery.retries >= 10 { return false; }
                    if delivery.last_sent.elapsed() >= Duration::from_millis(350) {
                        let _ = socket.send_to(&delivery.bytes, delivery.to);
                        delivery.retries += 1;
                        delivery.last_sent = Instant::now();
                    }
                    true
                });
            }
            for address in peers.keys().copied() {
                send(&socket, &MeshPacket::Goodbye { id }, address);
            }
        });
        Ok(Self {
            id, local_radmin_ips: ips, commands: tx, events: event_rx, voice_targets: targets,
            stopped,
        })
    }

    pub fn send_chat(&self, text: String) { let _ = self.commands.send(Command::Chat(text)); }
    pub fn add_peer(&self, addr: SocketAddr) { let _ = self.commands.send(Command::AddPeer(addr)); }
    pub fn voice(&self, joined: bool) { let _ = self.commands.send(Command::Voice(joined)); }
    pub fn rename(&self, nickname: String) { let _ = self.commands.send(Command::Rename(nickname)); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr_rule() {
        assert!(is_valid_address("26.12.1.2:45888".parse().unwrap()));
        assert!(is_valid_address("127.0.0.1:45888".parse().unwrap()));
        assert!(!is_valid_address("8.8.8.8:45888".parse().unwrap()));
    }

    #[test]
    fn mesh_survives_other_peer_exit() {
        // Three separate independent nodes, all on loopback, all on unique UDP ports.
        let a_addr: SocketAddr = "127.0.0.1:46501".parse().unwrap();
        let b_addr: SocketAddr = "127.0.0.1:46502".parse().unwrap();
        let c_addr: SocketAddr = "127.0.0.1:46503".parse().unwrap();
        let a = Mesh::start_on("Alice".into(), vec![b_addr,c_addr], 46501, false).unwrap();
        let b = Mesh::start_on("Bob".into(), vec![a_addr,c_addr], 46502, false).unwrap();
        let c = Mesh::start_on("Charlie".into(), vec![a_addr,b_addr], 46503, false).unwrap();
        let find_peers = |mesh: &Mesh, expected: usize| {
            let until = Instant::now() + Duration::from_secs(6);
            while Instant::now() < until {
                if let Ok(MeshEvent::Peers(peers)) = mesh.events.recv_timeout(Duration::from_millis(150)) {
                    if peers.len() == expected { return true; }
                }
            }
            false
        };
        assert!(find_peers(&a, 2));
        assert!(find_peers(&c, 2));
        drop(b);
        // Charlie and Alice remain directly linked; they can still exchange messages.
        thread::sleep(Duration::from_millis(300));
        a.send_chat("Still here!".into());
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut got_chat = false;
        while Instant::now() < deadline {
            if let Ok(MeshEvent::Chat { name, text }) = c.events.recv_timeout(Duration::from_millis(150)) {
                if name == "Alice" && text == "Still here!" {
                    got_chat = true; break;
                }
            }
        }
        assert!(got_chat, "remaining nodes should communicate without Bob");
    }
}
