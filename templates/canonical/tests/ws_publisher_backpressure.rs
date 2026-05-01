use canonical::{
    structs::ws::SubscriberHandle,
    transport::ws::{publisher::publish, registry::Registry},
};
use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Serialize)]
struct Tick {
    n: u32,
}

#[tokio::test]
async fn publish_drops_subscriber_when_channel_full() {
    let registry = Registry::new();
    let (tx, _rx) = mpsc::channel::<String>(1);
    let id = registry.next_id();
    registry.subscribe("topic:full".to_string(), SubscriberHandle { id, sender: tx.clone() });
    assert_eq!(registry.subscribers_count("topic:full"), 1);

    let delivered = publish(&registry, "topic:full", &Tick { n: 1 });
    assert_eq!(delivered, 1, "first publish fits in capacity-1 channel");
    assert_eq!(registry.subscribers_count("topic:full"), 1, "still subscribed after first publish");

    let delivered = publish(&registry, "topic:full", &Tick { n: 2 });
    assert_eq!(delivered, 0, "second publish hits Full and is not delivered");
    assert_eq!(registry.subscribers_count("topic:full"), 0, "subscriber evicted on Full");
}

#[tokio::test]
async fn publish_drops_subscriber_when_channel_closed() {
    let registry = Registry::new();
    let (tx, rx) = mpsc::channel::<String>(4);
    let id = registry.next_id();
    registry.subscribe("topic:closed".to_string(), SubscriberHandle { id, sender: tx.clone() });
    assert_eq!(registry.subscribers_count("topic:closed"), 1);

    drop(rx);

    let delivered = publish(&registry, "topic:closed", &Tick { n: 1 });
    assert_eq!(delivered, 0, "publish to closed channel delivers nothing");
    assert_eq!(registry.subscribers_count("topic:closed"), 0, "subscriber evicted on Closed");
}

#[tokio::test]
async fn publish_with_mixed_healthy_and_full_subscribers() {
    let registry = Registry::new();
    let (tx_healthy, mut rx_healthy) = mpsc::channel::<String>(8);
    let id_healthy = registry.next_id();
    registry.subscribe("topic:mix".to_string(), SubscriberHandle { id: id_healthy, sender: tx_healthy });

    let (tx_slow, _rx_slow) = mpsc::channel::<String>(1);
    let id_slow = registry.next_id();
    registry.subscribe("topic:mix".to_string(), SubscriberHandle { id: id_slow, sender: tx_slow.clone() });

    tx_slow.try_send("preload".to_string()).expect("preload occupies slow channel");

    let delivered = publish(&registry, "topic:mix", &Tick { n: 1 });
    assert_eq!(delivered, 1, "healthy gets it; slow does not");
    assert_eq!(registry.subscribers_count("topic:mix"), 1, "slow evicted, healthy survives");

    let received = rx_healthy.recv().await.expect("healthy received the frame");
    assert!(received.contains("\"topic\":\"topic:mix\""), "healthy frame contains topic");
}
