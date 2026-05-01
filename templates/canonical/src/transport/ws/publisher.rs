use std::{marker::PhantomData, sync::Arc};

use serde::Serialize;
use tokio::sync::mpsc::error::TrySendError;

use crate::{
    cata_log,
    structs::ws::{publisher::Channel, registry::{Registry, SubscriberId}},
    transport::ws::protocol::ServerMessage,
};

pub fn publish<T: Serialize>(registry: &Registry, topic: &str, event: &T) -> usize {
    let payload = match serde_json::to_value(event) {
        Ok(v) => v,
        Err(e) => {
            cata_log!(Warning, format!("publish: serialize event failed for topic '{}': {}", topic, e));
            return 0;
        }
    };
    let frame = ServerMessage::event(topic, payload);
    let encoded = match serde_json::to_string(&frame) {
        Ok(s) => s,
        Err(e) => {
            cata_log!(Warning, format!("publish: encode frame failed for topic '{}': {}", topic, e));
            return 0;
        }
    };

    let subscribers = registry.subscribers_of(topic);
    let mut delivered = 0;
    let mut to_drop: Vec<SubscriberId> = Vec::new();
    for sub in subscribers {
        match sub.sender.try_send(encoded.clone()) {
            Ok(()) => delivered += 1,
            Err(TrySendError::Full(_)) => {
                cata_log!(Warning, format!("publish: subscriber {} channel full on '{}'; dropping subscriber", sub.id, topic));
                to_drop.push(sub.id);
            }
            Err(TrySendError::Closed(_)) => {
                cata_log!(Debug, format!("publish: subscriber {} channel closed on '{}'", sub.id, topic));
                to_drop.push(sub.id);
            }
        }
    }
    for id in to_drop {
        registry.unsubscribe_all(id);
    }
    delivered
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

pub fn channel<T: Serialize + Send + Sync + 'static>(registry: Arc<Registry>, topic: impl Into<String>) -> Channel<T> {
    Channel {
        topic: topic.into(),
        registry,
        _phantom: PhantomData,
    }
}
