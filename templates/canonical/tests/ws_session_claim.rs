use canonical::transport::ws::registry::Registry;
use tokio::sync::oneshot;

#[tokio::test]
async fn claim_session_first_returns_none() {
    let registry = Registry::new();
    let (tx, _rx) = oneshot::channel::<()>();
    let prev = registry.claim_session(42, 1, tx);
    assert!(prev.is_none(), "first claim for a uid must return None");
}

#[tokio::test]
async fn claim_session_second_returns_prior_entry() {
    let registry = Registry::new();
    let (tx_first, _rx_first) = oneshot::channel::<()>();
    registry.claim_session(42, 1, tx_first);

    let (tx_second, _rx_second) = oneshot::channel::<()>();
    let prev = registry.claim_session(42, 2, tx_second).expect("second claim returns prior");
    assert_eq!(prev.subscriber_id, 1, "prior entry carries the OLD subscriber_id");
}

#[tokio::test]
async fn claim_session_different_uids_independent() {
    let registry = Registry::new();
    let (tx_a, _rx_a) = oneshot::channel::<()>();
    let (tx_b, _rx_b) = oneshot::channel::<()>();
    assert!(registry.claim_session(10, 1, tx_a).is_none());
    assert!(registry.claim_session(20, 2, tx_b).is_none(), "different uid does not collide");
}

#[tokio::test]
async fn release_session_matching_subscriber_id_removes() {
    let registry = Registry::new();
    let (tx, _rx) = oneshot::channel::<()>();
    registry.claim_session(42, 1, tx);
    registry.release_session(42, 1);

    let (tx_again, _rx_again) = oneshot::channel::<()>();
    let prev = registry.claim_session(42, 99, tx_again);
    assert!(prev.is_none(), "release dropped the entry; re-claim is fresh");
}

#[tokio::test]
async fn release_session_nonmatching_subscriber_id_preserves_newer_claim() {
    let registry = Registry::new();
    let (tx_a, _rx_a) = oneshot::channel::<()>();
    registry.claim_session(42, 1, tx_a);
    let (tx_b, _rx_b) = oneshot::channel::<()>();
    registry.claim_session(42, 2, tx_b);

    registry.release_session(42, 1);

    let (tx_c, _rx_c) = oneshot::channel::<()>();
    let prev = registry.claim_session(42, 3, tx_c).expect("uid 42 still mapped to subscriber 2");
    assert_eq!(prev.subscriber_id, 2, "stale release_session(uid, 1) must NOT strip subscriber 2");
}

#[tokio::test]
async fn evict_signal_fires_on_second_claim() {
    let registry = Registry::new();
    let (tx_first, mut rx_first) = oneshot::channel::<()>();
    registry.claim_session(42, 1, tx_first);

    let (tx_second, _rx_second) = oneshot::channel::<()>();
    let prev = registry.claim_session(42, 2, tx_second).expect("second claim returns prior");

    match prev.close_signal.send(()) {
        Ok(()) => {}
        Err(()) => panic!("first claim's receiver still alive — send must succeed"),
    }
    let evict = (&mut rx_first).await.expect("first claim's receiver got the close signal");
    let _ = evict;
}
