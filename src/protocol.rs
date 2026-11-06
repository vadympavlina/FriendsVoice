use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Packet {
    Hello { nickname: String, password: String },
    Welcome { users: Vec<String> },
    Chat { from: String, text: String },
    SendChat { text: String },
    Users { users: Vec<String> },
    Notice { text: String },
    Error { text: String },
}
