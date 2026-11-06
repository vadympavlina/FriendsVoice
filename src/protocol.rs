use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// This is a decentralized mesh. Each client owns its control and voice UDP sockets.
pub const CONTROL_PORT: u16 = 45888;
pub const AUDIO_PORT: u16 = 45889;
pub const VIDEO_PORT: u16 = 45890;
pub const MAX_NODES: usize = 5;
/// Media packet layout changed in 0.8: every participant must upgrade.
pub const PROTOCOL_VERSION: u16 = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MeshPacket {
    Hello { id: u64, name: String, voice: bool, #[serde(default)] protocol: u16 },
    Peers { id: u64, addresses: Vec<SocketAddr> },
    Goodbye { id: u64 },
    Chat { id: u64, seq: u64, name: String, text: String },
    Ack { id: u64, seq: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_roundtrip() {
        let hello = MeshPacket::Hello { id: 99, name: "Вадим".into(), voice: true, protocol: PROTOCOL_VERSION };
        let parsed = serde_json::from_slice::<MeshPacket>(&serde_json::to_vec(&hello).unwrap()).unwrap();
        assert!(matches!(parsed, MeshPacket::Hello { id: 99, voice: true, protocol: PROTOCOL_VERSION, .. }));
        let legacy = br#"{"type":"hello","id":99,"name":"Old","voice":true}"#;
        let parsed: MeshPacket = serde_json::from_slice(legacy).unwrap();
        assert!(matches!(parsed, MeshPacket::Hello { protocol: 0, .. }));
    }
}
