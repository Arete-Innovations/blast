use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ControlFrame {
    Ack { topic: String },

    Error {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        topic: Option<String>,
        reason: String,
    },

    Pong,
}
