use canonical::{
    structs::ws::SubscriberHandle,
    transport::ws::{publisher::publish, registry::Registry},
};
use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Serialize)]
struct TestEvent {
    kind: &'static str,
}

#[tokio::test]
async fn publish_fans_out_to_each_subscriber() {
    let registry = Registry::new();
    let (tx1, mut rx1) = mpsc::channel(4);
    let (tx2, mut rx2) = mpsc::channel(4);
    let id1 = registry.next_id();
    let id2 = registry.next_id();
    registry.subscribe("x:y:1".to_string(), SubscriberHandle { id: id1, sender: tx1 });
    registry.subscribe("x:y:1".to_string(), SubscriberHandle { id: id2, sender: tx2 });

    let n = publish(&registry, "x:y:1", &TestEvent { kind: "K" });
    assert_eq!(n, 2);

    let f1 = rx1.recv().await.unwrap();
    let f2 = rx2.recv().await.unwrap();
    assert_eq!(f1, f2);
    assert!(f1.contains("\"topic\":\"x:y:1\""));
    assert!(f1.contains("\"kind\":\"K\""));
}

#[tokio::test]
async fn publish_to_unknown_topic_is_zero() {
    let registry = Registry::new();
    let n = publish(&registry, "no:such:topic", &TestEvent { kind: "K" });
    assert_eq!(n, 0);
}

#[tokio::test]
async fn unsubscribe_all_removes_from_every_topic() {
    let registry = Registry::new();
    let (tx, _rx) = mpsc::channel(4);
    let id = registry.next_id();
    let handle = SubscriberHandle { id, sender: tx };
    registry.subscribe("a:b:1".to_string(), handle.clone());
    registry.subscribe("a:b:2".to_string(), handle);
    assert_eq!(registry.subscribers_count("a:b:1"), 1);
    assert_eq!(registry.subscribers_count("a:b:2"), 1);
    registry.unsubscribe_all(id);
    assert_eq!(registry.subscribers_count("a:b:1"), 0);
    assert_eq!(registry.subscribers_count("a:b:2"), 0);
}
