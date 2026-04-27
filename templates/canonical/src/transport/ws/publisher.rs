
use std::marker::PhantomData;
use std::sync::Arc;

use serde::Serialize;

use crate::cata_log;
use crate::structs::ws::publisher::Channel;
use crate::structs::ws::registry::Registry;
use super::protocol::ServerMessage;

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
    for sub in subscribers {
        if sub.sender.try_send(encoded.clone()).is_ok() {
            delivered += 1;
        }
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
