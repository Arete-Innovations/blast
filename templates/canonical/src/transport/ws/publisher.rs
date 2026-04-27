
use std::marker::PhantomData;
use std::sync::Arc;

use serde::Serialize;

use super::protocol::ServerMessage;
use super::registry::Registry;

pub fn publish<T: Serialize>(registry: &Registry, topic: &str, event: &T) -> usize {
    let payload = match serde_json::to_value(event) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let frame = ServerMessage::event(topic, payload);
    let encoded = match serde_json::to_string(&frame) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let subscribers = registry.subscribers_of(topic);
    let mut delivered = 0;
    for sub in subscribers {
        if sub.sender.try_send(encoded.clone()).is_ok() {
            delivered += 1;
        }
    }
    delivered
}

pub struct Channel<T: Serialize + Send + Sync + 'static> {
    topic: String,
    registry: Arc<Registry>,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: Serialize + Send + Sync + 'static> Channel<T> {
    pub fn publish(&self, event: &T) -> usize {
        publish(&self.registry, &self.topic, event)
    }

    pub fn subscribers_count(&self) -> usize {
        self.registry.subscribers_count(&self.topic)
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }
}

pub fn channel<T: Serialize + Send + Sync + 'static>(
    registry: Arc<Registry>,
    topic: impl Into<String>,
) -> Channel<T> {
    Channel {
        topic: topic.into(),
        registry,
        _phantom: PhantomData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::ws::registry::{Registry, SubscriberHandle};
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
        registry.subscribe(
            "x:y:1".to_string(),
            SubscriberHandle { id: id1, sender: tx1 },
        );
        registry.subscribe(
            "x:y:1".to_string(),
            SubscriberHandle { id: id2, sender: tx2 },
        );

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
}
