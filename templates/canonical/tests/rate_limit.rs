use std::time::Duration;

use canonical::structs::services::rate_limit::RateLimit;

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
