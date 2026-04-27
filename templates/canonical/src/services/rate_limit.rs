
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::structs::services::rate_limit::*;

impl Default for RateLimit {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimit {
    pub fn new() -> RateLimit {
        RateLimit { buckets: DashMap::new() }
    }

    pub fn check_and_consume(&self, key: &str, max: u32, window: Duration) -> bool {
        if max == 0 || window.is_zero() {
            return false;
        }

        let now = Instant::now();

        let mut entry = self
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket { tokens: max, last_refill: now });

        let bucket = entry.value_mut();

        let elapsed = now.saturating_duration_since(bucket.last_refill);
        if !elapsed.is_zero() {
            let refill_secs = elapsed.as_secs_f64();
            let rate_per_sec = max as f64 / window.as_secs_f64();
            let accrued = (refill_secs * rate_per_sec).floor() as u32;
            if accrued > 0 {
                bucket.tokens = bucket.tokens.saturating_add(accrued).min(max);
            }
            bucket.last_refill = now;
        }

        if bucket.tokens >= 1 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max_then_denies() {
        let rl = RateLimit::new();
        let key = "user:1";
        for _ in 0..5 {
            assert!(rl.check_and_consume(key, 5, Duration::from_secs(60)));
        }
        assert!(!rl.check_and_consume(key, 5, Duration::from_secs(60)));
    }

    #[test]
    fn refills_over_time() {
        let rl = RateLimit::new();
        let key = "user:2";
        for _ in 0..3 {
            assert!(rl.check_and_consume(key, 3, Duration::from_millis(300)));
        }
        assert!(!rl.check_and_consume(key, 3, Duration::from_millis(300)));
        std::thread::sleep(Duration::from_millis(350));
        assert!(rl.check_and_consume(key, 3, Duration::from_millis(300)));
    }

    #[test]
    fn separate_keys_dont_share() {
        let rl = RateLimit::new();
        for _ in 0..2 {
            assert!(rl.check_and_consume("a", 2, Duration::from_secs(60)));
        }
        assert!(!rl.check_and_consume("a", 2, Duration::from_secs(60)));
        assert!(rl.check_and_consume("b", 2, Duration::from_secs(60)));
    }

    #[test]
    fn zero_max_denies() {
        let rl = RateLimit::new();
        assert!(!rl.check_and_consume("k", 0, Duration::from_secs(1)));
    }
}
