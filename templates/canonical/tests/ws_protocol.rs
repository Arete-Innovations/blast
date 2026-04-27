use canonical::structs::ws::{ClientMessage, ServerMessage};
use serde_json::json;

#[test]
fn decodes_subscribe() {
    let m: ClientMessage = serde_json::from_str(r#"{"op":"subscribe","topic":"orders:customer:42"}"#).unwrap();
    match m {
        ClientMessage::Subscribe { topic } => assert_eq!(topic, "orders:customer:42"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn decodes_unsubscribe_and_ping() {
    let u: ClientMessage = serde_json::from_str(r#"{"op":"unsubscribe","topic":"x:y:z"}"#).unwrap();
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
