use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::structs::ws::control_frame::ControlFrame;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerMessage {
    Event {
        topic: String,
        #[serde(rename = "event")]
        payload: Value,
    },

    Control(ControlFrame),
}
