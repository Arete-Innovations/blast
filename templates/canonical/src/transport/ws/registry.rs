
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;

pub use crate::structs::ws::registry::{OutboundFrame, Registry, SubscriberHandle, SubscriberId, Topic};

impl Registry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            topics: DashMap::new(),
            next_id: AtomicU64::new(1),
        })
    }

    pub fn next_id(&self) -> SubscriberId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn subscribe(&self, topic: Topic, handle: SubscriberHandle) {
        let mut entry = self.topics.entry(topic).or_default();
        if !entry.iter().any(|h| h.id == handle.id) {
            entry.push(handle);
        }
    }

    pub fn unsubscribe(&self, topic: &str, subscriber_id: SubscriberId) {
        let remove_topic = {
            let Some(mut entry) = self.topics.get_mut(topic) else {
                return;
            };
            entry.retain(|h| h.id != subscriber_id);
            entry.is_empty()
        };
        if remove_topic {
            self.topics.remove(topic);
        }
    }

    pub fn unsubscribe_all(&self, subscriber_id: SubscriberId) {
        let mut empty_topics: Vec<Topic> = Vec::new();
        for mut entry in self.topics.iter_mut() {
            entry.retain(|h| h.id != subscriber_id);
            if entry.is_empty() {
                empty_topics.push(entry.key().clone());
            }
        }
        for topic in empty_topics {
            self.topics.remove(&topic);
        }
    }

    pub fn subscribers_of(&self, topic: &str) -> Vec<SubscriberHandle> {
        let Some(entry) = self.topics.get(topic) else {
            return Vec::new();
        };
        entry.clone()
    }

    pub fn subscribers_count(&self, topic: &str) -> usize {
        let Some(e) = self.topics.get(topic) else {
            return 0;
        };
        e.len()
    }
}
