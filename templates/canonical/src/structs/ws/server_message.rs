use crate::structs::ws::control_frame::ControlFrame;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
