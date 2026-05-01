use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use canonical::{
    crank::Crank,
    meltdown::{MeltDown, MeltType},
};

#[tokio::test]
async fn backoff_retries_transient_then_succeeds() {
    let attempts = Arc::new(AtomicU32::new(0));
    let crank = Crank::backoff(3, Duration::from_millis(1));

    let attempts_for_run = Arc::clone(&attempts);
    let result: Result<u32, MeltDown> = crank
        .run(|| {
            let attempts = Arc::clone(&attempts_for_run);
            async move {
                let n = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 2 {
                    Err(MeltDown::db_connection("first attempt flake"))
                } else {
                    Ok(42u32)
                }
            }
        })
        .await;

    assert!(result.is_ok(), "should succeed on second attempt: {:?}", result.err());
    assert_eq!(result.unwrap(), 42);
    assert_eq!(attempts.load(Ordering::SeqCst), 2, "exactly 2 attempts");
}

#[tokio::test]
async fn backoff_bails_immediately_on_permanent_error() {
    let attempts = Arc::new(AtomicU32::new(0));
    let crank = Crank::backoff(5, Duration::from_millis(1));

    let attempts_for_run = Arc::clone(&attempts);
    let result: Result<(), MeltDown> = crank
        .run(|| {
            let attempts = Arc::clone(&attempts_for_run);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(MeltDown::new(MeltType::SessionInvalid, "bad token"))
            }
        })
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_permanent(), "SessionInvalid is permanent");
    assert_eq!(attempts.load(Ordering::SeqCst), 1, "permanent err — single attempt, no retries");
}

#[tokio::test]
async fn backoff_gives_up_after_max_attempts_on_persistent_transient() {
    let attempts = Arc::new(AtomicU32::new(0));
    let crank = Crank::backoff(3, Duration::from_millis(1));

    let attempts_for_run = Arc::clone(&attempts);
    let result: Result<(), MeltDown> = crank
        .run(|| {
            let attempts = Arc::clone(&attempts_for_run);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(MeltDown::db_connection("always flakes"))
            }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 3, "exactly max_attempts when always transient");
}

#[tokio::test]
async fn none_does_not_retry_even_on_transient() {
    let attempts = Arc::new(AtomicU32::new(0));
    let crank = Crank::none();

    let attempts_for_run = Arc::clone(&attempts);
    let result: Result<(), MeltDown> = crank
        .run(|| {
            let attempts = Arc::clone(&attempts_for_run);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(MeltDown::db_connection("flake"))
            }
        })
        .await;

    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 1, "Crank::none never retries");
}
