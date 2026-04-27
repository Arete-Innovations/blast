use std::time::Instant;

use dashmap::DashMap;

pub struct RateLimit {
    pub(crate) buckets: DashMap<String, TokenBucket>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TokenBucket {
    pub(crate) tokens: u32,
    pub(crate) last_refill: Instant,
}
