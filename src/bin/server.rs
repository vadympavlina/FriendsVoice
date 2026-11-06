#[path = "../protocol.rs"]
mod protocol;
use protocol::{Packet, UserInfo, UDP_VOICE_TO_SERVER, UDP_VOICE_FROM_SERVER, VOICE_PORT_OFFSET};
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex, mpsc::{self, Sender}},
    thread,
};

struct Peer {
    name: String,
    token: u128,
    voice_addr: Option<SocketAddr>,
    tx: Sender<Packet>,
}
type Peers = Arc<Mutex<HashMap<u64, Peer>>>;

fn write_packet(stream: &mut TcpStream, packet: &Packet) -> std::io::Result<()> {
    let payload = serde_json::to_string(packet).map_err(std::io::Error::other)?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")
}

fn broadcast(peers: &Peers, packet: Packet) {
    let lock = peers.lock().unwrap();
    for peer in lock.values() {
        let _ = peer.tx.send(packet.clone());
    }
}

fn users(peers: &Peers) -> Vec<UserInfo> {
    let mut list: Vec<_> = peers.lock().unwrap().iter().map(|(id, peer)| UserInfo {
        id: *id,
        nickname: peer.name.clone(),
    }).collect();
    list.sort_by(|a, b| a.nickname.to_lowercase().cmp(&b.nickname.to_lowercase()));
    list
}

fn publish_users(peers: &Peers) {
    broadcast(peers, Packet::Users { users: users(peers) });
}

fn run_voice_relay(socket: UdpSocket, peers: Peers) {
    let mut data = [0u8; 1500];
    println!("Voice relay listening on UDP {}", socket.local_addr().unwrap());
    loop {
        let Ok((len, source)) = socket.recv_from(&mut data) else { continue; };
        if len < 18 || len > 1200 || data[0] != UDP_VOICE_TO_SERVER { continue; }
        let mut token_bytes = [0u8; 16];
        token_bytes.copy_from_slice(&data[1..17]);
        let token = u128::from_be_bytes(token_bytes);
        let (sender_id, targets) = {
            let mut state = peers.lock().unwrap();
            let sender = state.iter().find(|(_, p)| p.token == token).map(|(id, _)| *id);
            let Some(id) = sender else { continue; };
            if let Some(peer) = state.get_mut(&id) {
                peer.voice_addr = Some(source);
            }
            let targets = state.iter().filter_map(|(peer_id, peer)| {
                if *peer_id != id { peer.voice_addr } else { None }
            }).collect::<Vec<_>>();
            (id, targets)
        };
        let mut forwarded = Vec::with_capacity(len - 8);
        forwarded.push(UDP_VOICE_FROM_SERVER);
        forwarded.extend_from_slice(&sender_id.to_be_bytes());
        forwarded.extend_from_slice(&data[17..len]);
        for addr in targets {
            let _ = socket.send_to(&forwarded, addr);
        }
    }
}

fn handle(mut stream: TcpStream, id: u64, peers: Peers, password: String) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    let read_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(read_stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 || line.len() > 8192 { return; }
    let (name, supplied) = match serde_json::from_str::<Packet>(&line) {
        Ok(Packet::Hello { nickname, password }) => (nickname.trim().to_string(), password),
        _ => {
            let _ = write_packet(&mut stream, &Packet::Error { text: "Очікується авторизація".into() });
            return;
        }
    };
    if supplied != password || name.is_empty() || name.chars().count() > 24
        || name.chars().any(|c| c.is_control()) {
        let _ = write_packet(&mut stream, &Packet::Error { text: "Неправильний пароль або нікнейм".into() });
        return;
    }
    let (tx, rx) = mpsc::channel();
    let token = rand::random::<u128>();
    {
        let mut lock = peers.lock().unwrap();
        if lock.len() >= 5 || lock.values().any(|p| p.name.eq_ignore_ascii_case(&name)) {
            let _ = write_packet(&mut stream, &Packet::Error { text: "Кімната заповнена або нікнейм зайнятий".into() });
            return;
        }
        lock.insert(id, Peer { name: name.clone(), token, voice_addr: None, tx });
    }
    if write_packet(&mut stream, &Packet::Welcome {
        users: users(&peers),
        user_id: id,
        voice_token: format!("{token:032x}"),
    }).is_err() {
        peers.lock().unwrap().remove(&id);
        return;
    }
    let mut writer = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => {
            peers.lock().unwrap().remove(&id);
            return;
        }
    };
    thread::spawn(move || {
        while let Ok(packet) = rx.recv() {
            if write_packet(&mut writer, &packet).is_err() { break; }
        }
    });
    let _ = stream.set_read_timeout(None);
    println!("Joined: {name} (id={id})");
    broadcast(&peers, Packet::Notice { text: format!("{name} приєднався(-лася)") });
    publish_users(&peers);
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(n) if n > 8192 => break,
            Ok(_) => {
                if let Ok(Packet::SendChat { text }) = serde_json::from_str::<Packet>(&line) {
                    let text = text.trim();
                    if !text.is_empty() && text.chars().count() <= 1000 {
                        broadcast(&peers, Packet::Chat { from: name.clone(), text: text.to_string() });
                    }
                }
            }
        }
    }
    peers.lock().unwrap().remove(&id);
    broadcast(&peers, Packet::Notice { text: format!("{name} вийшов(-ла)") });
    publish_users(&peers);
    println!("Left: {name} (id={id})");
}

fn main() -> std::io::Result<()> {
    let args: Vec<_> = env::args().collect();
    let password = args.get(1).cloned().or_else(|| env::var("FRIENDS_VOICE_PASSWORD").ok()).unwrap_or_default();
    if password.chars().count() < 8 {
        eprintln!("Укажіть пароль від 8 символів: friends-voice-server.exe <пароль>");
        std::process::exit(2);
    }
    let bind = args.get(2).map(String::as_str).unwrap_or("0.0.0.0:45888");
    let listener = TcpListener::bind(bind)?;
    let addr = listener.local_addr()?;
    let voice_port = addr.port().checked_add(VOICE_PORT_OFFSET).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Voice port overflow")
    })?;
    let voice_addr = SocketAddr::new(addr.ip(), voice_port);
    let voice = UdpSocket::bind(voice_addr)?;
    let peers: Peers = Arc::new(Mutex::new(HashMap::new()));
    let voice_peers = peers.clone();
    thread::spawn(move || run_voice_relay(voice, voice_peers));
    println!("Friends Voice 0.2: TCP {}, UDP {}, максимум 5 друзів", addr, voice_addr);
    for (id, stream) in listener.incoming().enumerate() {
        match stream {
            Ok(stream) => {
                let p = peers.clone();
                let pass = password.clone();
                thread::spawn(move || handle(stream, id as u64 + 1, p, pass));
            }
            Err(e) => eprintln!("Помилка з'єднання: {e}"),
        }
    }
    Ok(())
}
