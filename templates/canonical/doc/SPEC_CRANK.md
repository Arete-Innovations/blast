# SPEC_CRANK

Retry combinator. Lives in `catalyst::crank`. Used from flows (not routines, not services).

## Metaphor

Engines crank the starter *until* they fire. `Crank::run(...)` invokes an async closure until it succeeds or policy exhausts.

## API

```rust
use catalyst::crank::{Crank, ExpBackoff};
use std::time::Duration;

let result = Crank::new(ExpBackoff::new(3, Duration::from_millis(200)).with_jitter())
    .classify(MeltDown::is_transient)
    .deadline(Duration::from_secs(10))
    .on_attempt(|n, e| tracing::warn!(attempt = n, error = ?e, "retrying"))
    .on_giveup(|n, e| tracing::error!(attempts = n, final_error = ?e, "gave up"))
    .run(|| routines::payments::charge(ctx, &card, amount))
    .await?;
```

## Builder Methods

| Method | Required | Meaning |
|--------|----------|---------|
| `Crank::new(policy)` | yes | Constructs with retry policy |
| `.classify(fn(&Error) -> bool)` | yes | Predicate: should this error be retried? |
| `.deadline(Duration)` | no | Overall time budget; abort retry loop regardless of attempts |
| `.on_attempt(fn(attempt, &Error))` | no | Hook fired before each retry (not before the first attempt) |
| `.on_giveup(fn(final_attempt, &Error))` | no | Hook fired when retries exhaust |
| `.run(closure)` | yes | Invokes the closure; returns `Result<T, E>` |

Closure signature: `FnMut() -> impl Future<Output = Result<T, E>>`. Called at least once; re-called on retryable failures.

## Policies

```rust
pub trait RetryPolicy {
    fn max_attempts(&self) -> usize;
    fn delay(&self, attempt: usize) -> Duration;
}
```

**Provided implementations:**

```rust

ExpBackoff::new(max_attempts: 3, base: Duration::from_millis(200))
    .with_jitter()
    .with_cap(Duration::from_secs(10))



FixedDelay::new(max_attempts: 5, delay: Duration::from_secs(1))


Immediate::new(max_attempts: 3)


impl RetryPolicy for MyPolicy {  }
```

## Classifier Pattern

```rust

.classify(MeltDown::is_transient)


.classify(|err: &MeltDown| matches!(err.melt_type,
    MeltType::DatabaseConnection | MeltType::ExternalServiceError))


.classify(|err: &MeltDown|
    err.is_transient()
    || matches!(err.melt_type, MeltType::TooManyRequests))
```

The classifier is called on every error. True → retry (if attempts left). False → return error immediately.

## Deadline Behavior

```rust
Crank::new(ExpBackoff::new(10, Duration::from_millis(200)))
    .deadline(Duration::from_secs(5))
    .classify(...)
    .run(|| ...)
    .await?;
```

After each attempt's failure, check `elapsed + next_delay >= deadline`. If so, return the last error without waiting or retrying.

## Hook Semantics

- `on_attempt(n, err)` fires BEFORE attempt `n` (so on retry, the first call doesn't trigger it; only retries do).
- `on_giveup(n, err)` fires once after all attempts exhaust (or deadline hits).

Hooks are synchronous to keep the combinator simple. Use them for logging, metrics, and telemetry — not for async side effects.

## Usage Rule: Flows Only

Retries are **operation-level** decisions, not implementation-level. The same service call should retry aggressively in one flow and not at all in another.

- **Flow:** wraps specific calls in `Crank`. Knows the business tolerance for failure.
- **Routine:** executes the actual work (calls models + services). Doesn't decide retry policy — that's the flow's job.
- **Service:** single-shot attempt. No retries internally.

## Example: Payment Flow

```rust

use catalyst::crank::{Crank, ExpBackoff};

pub async fn run(ctx: &Ctx, input: ChargeInput) -> Result<Receipt, MeltDown> {
    let receipt = Crank::new(
            ExpBackoff::new(3, Duration::from_millis(200))
                .with_jitter()
                .with_cap(Duration::from_secs(2))
        )
        .classify(MeltDown::is_transient)
        .deadline(Duration::from_secs(15))
        .on_attempt(|n, e| {
            tracing::warn!(attempt = n, ?e, "retrying charge");
        })
        .run(|| routines::payments::charge(ctx, &input.card, input.amount))
        .await?;

    Ok(receipt)
}
```

## Example: Reusable Helper (still used from flow)

```rust

pub async fn with_db_retry<T, F, Fut>(f: F) -> Result<T, MeltDown>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, MeltDown>>,
{
    Crank::new(ExpBackoff::new(3, Duration::from_millis(50)))
        .classify(|e: &MeltDown| matches!(e.melt_type,
            MeltType::DatabaseConnection))
        .run(f)
        .await
}
```

Flow calls the routine: `routines::orders::insert(ctx, &data).await?` — the routine internally wraps the models call in `with_db_retry`.

## Implementation Notes

- Hand-rolled in Catalyst. No `backoff` / `tryhard` external crate dep.
- Target ~150 LOC, zero transitive deps.
- Exact API matches Catablast needs — classifier, deadline, hooks, all first-class.
- If future needs outgrow the implementation, migrating to `backoff` is a one-flow-at-a-time change.

## Anti-Patterns

**Retrying in a service:**
```rust

pub async fn charge(card: &Card, amount: u64) -> Result<Receipt, MeltDown> {
    for attempt in 0..3 {
        match stripe_api_call(card, amount).await {
            Ok(r) => return Ok(r),
            Err(e) if attempt < 2 => tokio::time::sleep(...).await,
            Err(e) => return Err(e),
        }
    }
}
```

Service should single-shot. Caller (flow) wraps in `Crank`.

**Retrying validation errors:**
```rust

.classify(|_| true)
```

Always check what's actually retryable. Retrying a 400/422 wastes time and may duplicate side effects.

**Long retry loops in hot paths:**
```rust

Crank::new(ExpBackoff::new(10, Duration::from_secs(1)))
    .run(...)
```

Users see 30s latency. Pick short deadlines for sync request paths; long ones for background Fuses.

## Related Specs

- `SPEC_MELTDOWN.md` — `is_transient()`, `TooManyRequests`, `retry_after` fields
- `SPEC_FLOWS.md` — Crank lives in flows
- `SPEC_FUSES.md` — cron-like paths where long retry deadlines are acceptable
