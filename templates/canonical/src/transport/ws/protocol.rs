
pub use crate::structs::ws::{ClientMessage, ControlFrame, ServerMessage};
use serde_json::Value;

impl ServerMessage {
    pub fn ack(topic: impl Into<String>) -> Self {
        ServerMessage::Control(ControlFrame::Ack { topic: topic.into() })
    }

    pub fn error(topic: impl Into<String>, reason: impl Into<String>) -> Self {
        ServerMessage::Control(ControlFrame::Error {
            topic: Some(topic.into()),
            reason: reason.into(),
        })
    }

    pub fn error_global(reason: impl Into<String>) -> Self {
        ServerMessage::Control(ControlFrame::Error {
            topic: None,
            reason: reason.into(),
        })
    }

    pub fn pong() -> Self {
        ServerMessage::Control(ControlFrame::Pong)
    }

    pub fn event(topic: impl Into<String>, payload: Value) -> Self {
        ServerMessage::Event {
            topic: topic.into(),
            payload,
        }
    }
}
