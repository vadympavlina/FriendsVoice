#[path = "../src/protocol.rs"]
mod protocol;
use protocol::{Packet, UDP_VOICE_FROM_SERVER, UDP_VOICE_TO_SERVER};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn connect(port: u16) -> TcpStream {
    for _ in 0..60 {
        if let Ok(stream) = TcpStream::connect(("127.0.0.1", port)) {
            return stream;
        }
        thread::sleep(Duration::from_millis(150));
    }
    panic!("Server never opened TCP port {port}");
}

fn login(port: u16, name: &str) -> (TcpStream, BufReader<TcpStream>, u64, u128) {
    let mut stream = connect(port);
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let hello = Packet::Hello { nickname: name.into(), password: "test-password-123".into() };
    writeln!(stream, "{}", serde_json::to_string(&hello).unwrap()).unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    match serde_json::from_str::<Packet>(&line).unwrap() {
        Packet::Welcome { user_id, voice_token, .. } => (stream, reader, user_id, u128::from_str_radix(&voice_token, 16).unwrap()),
        unexpected => panic!("Unexpected handshake response {unexpected:?}"),
    }
}

#[test]
fn server_chat_and_authenticated_voice_relay() {
    let port = (46000u16..46100).find(|port| {
        if let Ok(tcp) = TcpListener::bind(("127.0.0.1", *port)) {
            if let Ok(udp) = UdpSocket::bind(("127.0.0.1", *port + 1)) {
                drop(udp);
                drop(tcp);
                return true;
            }
        }
        false
    }).expect("No free TCP + UDP port pair");

    let process = Command::new(env!("CARGO_BIN_EXE_friends-voice-server"))
        .args(["test-password-123", &format!("127.0.0.1:{port}")])
        .stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().unwrap();
    let _server = Server(process);

    let (mut a, mut reader_a, id_a, token_a) = login(port, "Alice");
    let (_b, reader_b, id_b, token_b) = login(port, "Bob");
    // Leave both clients idle for longer than the old 250ms client socket timeout.
    // Both must stay logged in and still exchange chat afterwards.
    thread::sleep(Duration::from_secs(2));
    assert_ne!(id_a, id_b);
    assert_ne!(token_a, token_b);

    let chat = Packet::SendChat { text: "hello".into() };
    writeln!(a, "{}", serde_json::to_string(&chat).unwrap()).unwrap();
    let mut got_chat = false;
    for _ in 0..12 {
        let mut line = String::new();
        reader_a.read_line(&mut line).unwrap();
        if let Packet::Chat { from, text } = serde_json::from_str::<Packet>(&line).unwrap() {
            assert_eq!(from, "Alice");
            assert_eq!(text, "hello");
            got_chat = true;
            break;
        }
    }
    assert!(got_chat, "Chat broadcast did not arrive");

    let udp_a = UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp_b = UdpSocket::bind("127.0.0.1:0").unwrap();
    udp_b.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    let endpoint = ("127.0.0.1", port + 1);
    let audio_packet = |token: u128, marker: u8| {
        let mut msg = vec![UDP_VOICE_TO_SERVER];
        msg.extend_from_slice(&token.to_be_bytes());
        msg.extend_from_slice(&[marker, 10, 11, 12]);
        msg
    };
    // First packet registers Bob's verified UDP endpoint.
    udp_b.send_to(&audio_packet(token_b, 88), endpoint).unwrap();
    thread::sleep(Duration::from_millis(120));
    udp_a.send_to(&audio_packet(token_a, 77), endpoint).unwrap();

    let mut received = [0u8; 100];
    let (size, addr) = udp_b.recv_from(&mut received).unwrap();
    assert_eq!(addr.port(), port + 1);
    assert_eq!(received[0], UDP_VOICE_FROM_SERVER);
    assert_eq!(u64::from_be_bytes(received[1..9].try_into().unwrap()), id_a);
    assert_eq!(&received[9..size], &[77, 10, 11, 12]);

    // A third unauthenticated socket cannot inject audio into the room.
    let rogue = UdpSocket::bind("127.0.0.1:0").unwrap();
    rogue.send_to(&audio_packet(0, 99), endpoint).unwrap();
    udp_b.set_read_timeout(Some(Duration::from_millis(350))).unwrap();
    assert!(udp_b.recv_from(&mut received).is_err());
    // Touch the second TCP connection so it remains active throughout the test.
    let _ = reader_b.get_ref().peer_addr().unwrap();
}
