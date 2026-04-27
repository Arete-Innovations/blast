use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe { topic: String },

    Unsubscribe { topic: String },

    Ping,
}
