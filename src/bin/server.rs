#[path = "../protocol.rs"]
mod protocol;
use protocol::Packet;
use std::{
    collections::HashMap,
    env,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex, mpsc::{self, Sender}},
    thread,
    time::Duration,
};

struct Peer { name: String, tx: Sender<Packet> }
type Peers = Arc<Mutex<HashMap<u64, Peer>>>;

fn send(stream: &mut TcpStream, packet: &Packet) -> std::io::Result<()> {
    let data = serde_json::to_string(packet).map_err(std::io::Error::other)?;
    stream.write_all(data.as_bytes())?;
    stream.write_all(b"\n")
}

fn broadcast(peers: &Peers, packet: Packet) {
    let lock = peers.lock().unwrap();
    for peer in lock.values() {
        let _ = peer.tx.send(packet.clone());
    }
}

fn publish_users(peers: &Peers) {
    let mut users: Vec<_> = peers.lock().unwrap().values().map(|p| p.name.clone()).collect();
    users.sort();
    broadcast(peers, Packet::Users { users });
}

fn handle(mut stream: TcpStream, id: u64, peers: Peers, password: String) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(reader_stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 { return; }
    if line.len() > 8192 { return; }
    let (name, supplied) = match serde_json::from_str::<Packet>(&line) {
        Ok(Packet::Hello { nickname, password }) => (nickname.trim().to_string(), password),
        _ => {
            let _ = send(&mut stream, &Packet::Error { text: "Очікується авторизація".into() });
            return;
        }
    };
    if supplied != password || name.is_empty() || name.chars().count() > 24 || name.chars().any(|c| c.is_control()) {
        let _ = send(&mut stream, &Packet::Error { text: "Неправильний пароль або нікнейм".into() });
        return;
    }
    let (tx, rx) = mpsc::channel();
    {
        let mut lock = peers.lock().unwrap();
        if lock.len() >= 5 || lock.values().any(|p| p.name.eq_ignore_ascii_case(&name)) {
            let _ = send(&mut stream, &Packet::Error { text: "Кімната заповнена або нікнейм зайнятий".into() });
            return;
        }
        lock.insert(id, Peer { name: name.clone(), tx });
    }
    let mut users: Vec<_> = peers.lock().unwrap().values().map(|p| p.name.clone()).collect();
    users.sort();
    if send(&mut stream, &Packet::Welcome { users }).is_err() {
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
        while let Ok(p) = rx.recv() {
            if send(&mut writer, &p).is_err() { break; }
        }
    });
    let _ = stream.set_read_timeout(None);
    broadcast(&peers, Packet::Notice { text: format!("{name} приєднався") });
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
    broadcast(&peers, Packet::Notice { text: format!("{name} вийшов") });
    publish_users(&peers);
}

fn main() -> std::io::Result<()> {
    let args: Vec<_> = env::args().collect();
    let password = args.get(1).cloned()
        .or_else(|| env::var("FRIENDS_VOICE_PASSWORD").ok())
        .unwrap_or_default();
    if password.len() < 8 {
        eprintln!("Вкажіть пароль мінімум 8 символів: friends-voice-server.exe МОЇЙ_ПАРОЛЬ");
        std::process::exit(2);
    }
    let bind = args.get(2).map(String::as_str).unwrap_or("0.0.0.0:45888");
    let listener = TcpListener::bind(bind)?;
    println!("Friends Voice сервер працює на {bind}; максимум 5 учасників");
    let peers: Peers = Arc::new(Mutex::new(HashMap::new()));
    for (id, stream) in listener.incoming().enumerate() {
        match stream {
            Ok(stream) => {
                let p = peers.clone();
                let pass = password.clone();
                thread::spawn(move || handle(stream, id as u64, p, pass));
            }
            Err(e) => eprintln!("Помилка підключення: {e}"),
        }
    }
    Ok(())
}
