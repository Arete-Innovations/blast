# SPEC_CRANK

Retry combinator. Lives in `crate::crank`. Used **from flows only** — enforced by `LAYER:11`–`LAYER:17` in `build.rs` (every non-flow layer has `crate::crank` in its banned-import list).

## Metaphor

Engines crank the starter *until* they fire. `Crank::run(...)` invokes an async closure until it succeeds or the policy exhausts.

## Constructors (batteries included)

```rust
Crank::none()                                  // single attempt, no retry
Crank::backoff(3, Duration::from_millis(500))  // exp backoff, default classifier
Crank::fixed(5, Duration::from_secs(1))        // fixed delay, default classifier
Crank::new(custom_policy)                      // escape hatch — requires .classify(...)
```

The first three set a default classifier; `Crank::new` requires `.classify(...)` explicitly (panics on `.run()` if missing).

## Default classifier

`Crank::backoff` and `Crank::fixed` default to:

```rust
|e: &MeltDown| !e.is_permanent()
```

That is: **retry on every error EXCEPT obviously-permanent ones**. Permanent set (see `MeltDown::is_permanent()`):

- Validation: `ValidationFailed`, `BadRequest`, `UnprocessableEntity`, `MethodNotAllowed`
- Auth: `AuthRejected`, `Unauthorized`, `Forbidden`, `InsufficientPermissions`
- Sessions: `SessionMissing`, `SessionInvalid`, `SessionExpired`
- Not found: `NotFound`, `RecordNotFound`, `FileNotFound`
- Conflicts: `Conflict`, `UniqueViolation`, `ForeignKeyViolation`, `CheckViolation`, `NotNullViolation`
- Permission: `FilePermissionDenied`
- Marshaling: `SerializationFailed`, `DeserializationFailed`
- Setup: `ConfigurationError`, `EnvironmentError`

Everything else (DB blips, external service hiccups, IO errors, `Unexpected`) retries by default. `Crank::none()` keeps its `|_| false` classifier — never retries.

## Builder methods (override the classifier)

| Method | Sets classifier to |
|--------|-------------------|
| `.classify(closure)` | the closure (full control) |
| `.retry_only_transient()` | `\|e\| e.is_transient()` (strict — only `DatabaseConnection` / `ExternalServiceError` / `TooManyRequests`) |
| `.retry_all()` | `\|_\| true` (YOLO — every Err) |

Chain in builder style:

```rust
Crank::backoff(3, Duration::from_millis(500))
    .retry_only_transient()
    .deadline(Duration::from_secs(10))
    .run(|| routines::custom::stripe::charge(ctx, &input))
    .await?;
```

## Retry-After override

When the routine returns `Err(MeltDown { retry_after: Some(d), .. })`, Crank uses `d` as the next sleep duration **instead of** the policy's `delay(attempt_no)`. Useful when an upstream API sends a `Retry-After` header (429, 503).

The service layer parses the header and constructs the MeltDown with `retry_after` set. Crank reads it transparently. Service code never imports `crank`.

Helper for parsing inbound `Retry-After`:

```rust
use crate::services::external_http::parse_retry_after;

let resp = reqwest::Client::new().post(url).send().await
    .map_err(|e| MeltDown::external_service(format!("upstream: {}", e)))?;

if resp.status() == 429 {
    let retry = parse_retry_after(&resp).unwrap_or(Duration::from_secs(60));
    return Err(MeltDown::too_many_requests(retry));
}
```

(`unwrap_or` shown for brevity — actual code uses `match` to comply with `ERROR:3`.)

## Builder Methods (full surface)

| Method | Required | Meaning |
|--------|----------|---------|
| `Crank::none()` | — | single-attempt, no retry. Most common. |
| `Crank::backoff(attempts, base)` | — | exp backoff with default classifier |
| `Crank::fixed(attempts, delay)` | — | fixed delay with default classifier |
| `Crank::new(policy)` | yes | full control — must chain `.classify(...)` |
| `.classify(closure)` | depends | override classifier |
| `.retry_only_transient()` | — | shortcut classifier |
| `.retry_all()` | — | shortcut classifier |
| `.deadline(Duration)` | — | overall time budget |
| `.on_attempt(closure)` | — | hook fired before each retry (not before first attempt) |
| `.on_giveup(closure)` | — | hook fired when retries exhaust |
| `.run(closure)` | yes | invokes; returns `Result<T, MeltDown>` |

Closure signature: `FnMut() -> impl Future<Output = Result<T, MeltDown>>`. Called at least once; re-called on retryable failures.

## Policies (low-level)

Direct policy types are available for `Crank::new(...)` callers:

```rust
ExpBackoff::new(max_attempts, base)
    .with_jitter()
    .with_cap(Duration::from_secs(10))

FixedDelay::new(max_attempts, delay)

Immediate::new(max_attempts)
```

Custom policies implement `RetryPolicy`:

```rust
pub trait RetryPolicy {
    fn max_attempts(&self) -> usize;
    fn delay(&self, attempt: usize) -> Duration;
}
```

## Deadline Behavior

After each attempt's failure, Crank checks `elapsed + next_delay >= deadline`. If so, returns the last error without waiting or retrying. Use deadlines aggressively on user-facing flows; relax on background fuses.

## Hook Semantics

- `on_attempt(n, err)` fires BEFORE attempt `n` (so the first call doesn't trigger it; only retries do).
- `on_giveup(n, err)` fires once after all attempts exhaust (or deadline hits).

Hooks are synchronous — keep them tiny. Use them for logging, metrics, telemetry — not async side effects.

## Usage Rule: Flows Only

Retries are **operation-level** decisions, not implementation-level. The same routine can retry aggressively in one flow and not at all in another.

| Layer | Crank? |
|-------|--------|
| Flow | yes — owns retry policy |
| Routine | NO — single-shot atomic capability |
| Service | NO — single-shot adapter |
| Models / Database | NO |

Enforcement: `crate::crank` is on the banned-import list for every layer except `flows/`. The build fails at `LAYER:N` if anyone else imports it.

## Example: Payment Flow

```rust
use crate::crank::Crank;
use std::time::Duration;

pub async fn run(ctx: &Ctx, input: ChargeInput) -> Result<Receipt, MeltDown> {
    ctx.require_role(Role::Member)?;

    let receipt = Crank::backoff(3, Duration::from_millis(200))
        .deadline(Duration::from_secs(15))
        .on_attempt(|n, e| cata_log!(Warning, format!("retrying charge attempt {}: {}", n, e)))
        .run(|| routines::custom::payments::charge(ctx, &input.card, input.amount))
        .await?;

    Ok(receipt)
}
```

## Example: Trivial flow (registration with no retry)

```rust
pub async fn run(ctx: &Ctx, input: RegisterInput) -> Result<UserPublic, MeltDown> {
    ctx.require_anonymous()?;
    Crank::none()
        .run(|| routines::custom::auth::register::run(ctx, input))
        .await
}
```

`Crank::none()` is mandatory even when no retry is desired — the policy is part of the flow's declared contract. Future ops/observability tooling reads policies straight from the source.

## Anti-Patterns

**Retrying in a routine or service:**
Banned at compile time. `crate::crank` import outside `flows/` fails `LAYER:11–17`.

**Retrying validation errors:**
```rust
Crank::backoff(3, ...).retry_all().run(...)  // BAD — retries 400/422/Conflict pointlessly
```
Default classifier already excludes permanent errors. `.retry_all()` is YOLO mode — only use when you genuinely know retrying every error is correct.

**Long retry loops in hot paths:**
```rust
Crank::backoff(10, Duration::from_secs(1)).run(...)  // BAD on a 500ms-budget request
```
Set `.deadline(...)` aggressively for sync request paths. Long deadlines belong in `transport/fuses/` (background).

## Related Specs

- `SPEC_MELTDOWN.md` — `is_transient()`, `is_permanent()`, `retry_after`, `TooManyRequests`
- `SPEC_FLOWS.md` — flow body must declare a Crank policy
- `SPEC_FUSES.md` — long-deadline retries acceptable in scheduled tasks
- `SPEC_ARCHITECTURE.md` — layer rules; `crate::crank` is flows-only
