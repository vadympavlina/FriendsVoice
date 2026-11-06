use serde::{Deserialize, Serialize};

/// All text-control packets travel over TCP. Audio uses a separate authenticated UDP socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Packet {
    Hello { nickname: String, password: String },
    Welcome { users: Vec<UserInfo>, user_id: u64, voice_token: u128 },
    SendChat { text: String },
    Chat { from: String, text: String },
    Users { users: Vec<UserInfo> },
    Notice { text: String },
    Error { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: u64,
    pub nickname: String,
}

/// Client voice data: type 1, 16-byte random session token, then Opus payload.
/// Server voice data: type 2, 8-byte sender ID, then Opus payload.
pub const UDP_VOICE_TO_SERVER: u8 = 1;
pub const UDP_VOICE_FROM_SERVER: u8 = 2;
pub const VOICE_PORT_OFFSET: u16 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_serialization() {
        let packet = Packet::Welcome {
            users: vec![UserInfo { id: 3, nickname: "Друг".into() }],
            user_id: 3,
            voice_token: 123456789123456789123456789u128,
        };
        let json = serde_json::to_string(&packet).unwrap();
        let parsed: Packet = serde_json::from_str(&json).unwrap();
        match parsed {
            Packet::Welcome { users, user_id, voice_token } => {
                assert_eq!(user_id, 3);
                assert_eq!(voice_token, 123456789123456789123456789u128);
                assert_eq!(users[0].nickname, "Друг");
            }
            _ => panic!("wrong type"),
        }
    }
}
