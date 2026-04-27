
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::mpsc;

pub type Topic = String;

pub type SubscriberId = u64;

pub type OutboundFrame = String;

#[derive(Clone)]
pub struct SubscriberHandle {
    pub id: SubscriberId,
    pub sender: mpsc::Sender<OutboundFrame>,
}

pub struct Registry {
    topics: DashMap<Topic, Vec<SubscriberHandle>>,
    next_id: AtomicU64,
}

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
        let mut remove_topic = false;
        if let Some(mut entry) = self.topics.get_mut(topic) {
            entry.retain(|h| h.id != subscriber_id);
            remove_topic = entry.is_empty();
        }
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
        self.topics
            .get(topic)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    pub fn subscribers_count(&self, topic: &str) -> usize {
        self.topics.get(topic).map(|e| e.len()).unwrap_or(0)
    }
}
