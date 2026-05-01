use std::sync::atomic::AtomicU64;

use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};

pub type Topic = String;

pub type SubscriberId = u64;

pub type OutboundFrame = String;

#[derive(Clone)]
pub struct SubscriberHandle {
    pub id: SubscriberId,
    pub sender: mpsc::Sender<OutboundFrame>,
}

pub struct SessionEntry {
    pub subscriber_id: SubscriberId,
    pub close_signal: oneshot::Sender<()>,
}

pub struct Registry {
    pub topics: DashMap<Topic, Vec<SubscriberHandle>>,
    pub next_id: AtomicU64,
    pub sessions: DashMap<i64, SessionEntry>,
}
