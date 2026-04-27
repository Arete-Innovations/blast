
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe { topic: String },

    Unsubscribe { topic: String },

    Ping,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decodes_subscribe() {
        let m: ClientMessage =
            serde_json::from_str(r#"{"op":"subscribe","topic":"orders:customer:42"}"#).unwrap();
        match m {
            ClientMessage::Subscribe { topic } => assert_eq!(topic, "orders:customer:42"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn decodes_unsubscribe_and_ping() {
        let u: ClientMessage =
            serde_json::from_str(r#"{"op":"unsubscribe","topic":"x:y:z"}"#).unwrap();
        assert!(matches!(u, ClientMessage::Unsubscribe { .. }));
        let p: ClientMessage = serde_json::from_str(r#"{"op":"ping"}"#).unwrap();
        assert!(matches!(p, ClientMessage::Ping));
    }

    #[test]
    fn encodes_event_without_op_field() {
        let m = ServerMessage::event("orders:customer:42", json!({"type":"Changed"}));
        let s = serde_json::to_string(&m).unwrap();
        assert!(s.contains("\"topic\":\"orders:customer:42\""));
        assert!(s.contains("\"event\":{\"type\":\"Changed\"}"));
        assert!(!s.contains("\"op\""));
    }

    #[test]
    fn encodes_ack() {
        let s = serde_json::to_string(&ServerMessage::ack("x:y:1")).unwrap();
        assert!(s.contains("\"op\":\"ack\""));
        assert!(s.contains("\"topic\":\"x:y:1\""));
    }

    #[test]
    fn encodes_error_with_and_without_topic() {
        let s1 = serde_json::to_string(&ServerMessage::error("x", "forbidden")).unwrap();
        assert!(s1.contains("\"topic\":\"x\""));
        assert!(s1.contains("\"reason\":\"forbidden\""));
        let s2 = serde_json::to_string(&ServerMessage::error_global("unknown_op")).unwrap();
        assert!(!s2.contains("\"topic\""));
        assert!(s2.contains("\"reason\":\"unknown_op\""));
    }

    #[test]
    fn encodes_pong() {
        let s = serde_json::to_string(&ServerMessage::pong()).unwrap();
        assert_eq!(s, r#"{"op":"pong"}"#);
    }
}
