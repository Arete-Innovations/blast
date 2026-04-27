# SPEC_FLOWS

Flows are the capability inventory. `ls src/flows/` is the answer to "what can this app do?"

## Core Definition

A flow is a **named business operation registered with a retry policy**. One flow file == one named op. CLI, tests, HTTP, WebSockets, and scheduled Fuses all dispatch through the same flow namespace.

Concretely, every flow is built from:

1. an auth/role check on `&Ctx`,
2. one or more **routine** calls,
3. each wrapped in a `Crank` retry policy (`Crank::none()` if no retry),
4. returning `Result<T, MeltDown>`.

A flow is NOT:
- a wrapper around models or services (those are routine territory)
- a chain of other flows (banned — see hard rules)
- inline glue (private helpers belong in the routine, not the flow)

## Directory Layout

```
src/flows/
├── mod.rs
├── generated/              ← Blast emits from resource state files
│   ├── mod.rs
│   ├── users/
│   │   ├── mod.rs
│   │   ├── list.rs
│   │   ├── get.rs
│   │   ├── create.rs
│   │   ├── update.rs
│   │   └── delete.rs
│   └── orders/
│       └── ...
└── custom/                 ← hand-written business ops
    ├── mod.rs
    ├── signup.rs
    ├── checkout.rs
    └── cancel_subscription.rs
```

Generated flows: one subdir per resource, one file per declared verb. Custom flows: flat, one file per op. Both live side by side.

## Hard Rules

### 1. Transport calls flows only

```rust
pub async fn signup(
    State(ctx): State<Ctx>,
    Json(input): Json<SignupInput>,
) -> Result<Json<UserPublic>, MeltDown> {
    let user = flows::custom::signup::run(&ctx, input).await?;
    Ok(Json(user))
}
```

One handler → one flow call. No branching, no secondary calls.

### 2. Flows call routines ONLY

No `models::*`, no `services::*`, no `database::*` imports inside `flows/`. Every external operation goes through a routine. If the routine doesn't exist yet, **write the routine first**.

### 3. Flows do not call flows

`flows/foo` MAY NOT call `flows/bar`. If two flows need the same logic, that logic is a routine.

### 4. Every flow declares a `Crank` policy

Even when no retry is desired, the flow declares `Crank::none()` explicitly. This makes retry policy a first-class, audit-able fact per capability — `Arsenal` reads it directly off the flow source. There is no implicit "no retry" — write it.

### 5. Flow owns auth enforcement

Transport middleware delivers a session (or anonymous `Ctx`); the flow decides "can THIS session do THIS op?". Generated flows enforce per-verb auth from the resource state file. Custom flows hand-write the check at the top of `run()`.

### 6. One transport call == one flow call

If a use case needs multiple ops composed, **build a composite custom flow that chains routines** — never sequence flows from transport.

### 7. Trivial flows are legal

A flow that does `Crank::none() → one routine call → map result` is still a real flow. Its value is registration + policy + auth, not body complexity. Stop apologizing for short flows.

## Generated Flow Shape

For a resource declaring `list` with `auth_required`, `paginated`, `filtered_by: ["role"]`, Blast emits:

```rust
use crate::{
    crank::Crank,
    meltdown::MeltDown,
    routines,
    structs::{generated::users::*, common::Pagination},
    Ctx,
};

pub async fn run(
    ctx: &Ctx,
    pagination: Pagination,
    filters: UserListFilters,
) -> Result<PaginatedList<UserPublic>, MeltDown> {
    ctx.require_session()?;
    Crank::none()
        .run(|| routines::generated::users::list(ctx, &pagination, &filters))
        .await
}
```

Generated flows are predictable, ~10-15 lines, regeneration-safe. They wrap exactly one generated routine under `Crank::none()` plus the resource's declared auth check.

## Custom Flow Shape

Custom flows live in `flows/custom/`. They compose routines (never models/services/database) and may declare per-routine retry policies.

```rust
use crate::{
    crank::Crank,
    meltdown::MeltDown,
    routines,
    structs::users::*,
    Ctx,
};

use std::time::Duration;

pub struct SignupInput {
    pub email: String,
    pub password: String,
}

pub async fn run(ctx: &Ctx, input: SignupInput) -> Result<UserPublic, MeltDown> {
    ctx.require_anonymous()?;

    let user = Crank::none()
        .run(|| routines::custom::users::create(ctx, &input))
        .await?;

    Crank::backoff(3, Duration::from_millis(500))
        .run(|| routines::custom::email::welcome::send(&user.email))
        .await?;

    Ok(user.into_public())
}
```

Sequential. English-readable. `?`-propagates errors. Each step has its own visible Crank policy.

## Composing Flows (DON'T)

If you find yourself wanting two flows to fire in one route, build a **composite custom flow** that chains routines instead.

```rust
pub async fn run(ctx: &Ctx, input: CheckoutInput) -> Result<Receipt, MeltDown> {
    ctx.require_session()?;

    let charge = Crank::backoff(3, Duration::from_millis(200))
        .run(|| routines::custom::payments::charge(&input.card, input.amount))
        .await?;

    let sub = Crank::none()
        .run(|| routines::custom::subscriptions::create(ctx, &input, &charge))
        .await?;

    Crank::backoff(2, Duration::from_millis(500))
        .run(|| routines::custom::email::send_receipt(&sub.email, &charge))
        .await?;

    Crank::none()
        .run(|| routines::custom::users::update_tier(ctx, sub.user_id))
        .await?;

    Ok(Receipt::from((charge, sub)))
}
```

One route, one flow. The flow itself composes routines internally, each under its own retry policy.

## Fuses Dispatch a Flow

Scheduled tasks dispatch to exactly one flow per tick.

```rust
Fuse::every(Duration::from_minutes(5))
    .run(flows::custom::cleanup_expired_sessions::run)
    .register();
```

See `SPEC_FUSES.md`.

## WebSocket Handlers Call a Flow

Per WS message, one flow.

```rust
pub async fn on_message(ws: &WsCtx, msg: ChatSend) -> Result<(), MeltDown> {
    flows::custom::post_chat_message::run(&ws.ctx, msg).await?;
    Ok(())
}
```

See `SPEC_RELAY.md`.

## Anti-Patterns

**Flow calling models/services/database directly:**

```rust
pub async fn run(ctx: &Ctx, input: SignupInput) -> Result<User, MeltDown> {
    let hash = services::crypto::hash_password(&input.password)?;
    models::users::create(ctx.conn(), &NewUser { ... }).await
}
```

Banned. Both calls belong in a routine. The flow calls the routine under a `Crank` policy.

**Flow wrapping a flow:**

```rust
pub async fn signup(ctx: &Ctx, input: SignupInput) -> Result<UserPublic, MeltDown> {
    flows::generated::users::create::run(ctx, input.into()).await
}
```

Banned. Custom flows compose routines, not other flows.

**Multi-flow in a route:**

```rust
pub async fn signup_route(State(ctx), Json(input)) -> Result<...> {
    let user = flows::custom::create_user::run(...).await?;
    flows::custom::send_welcome::run(...).await?;
    flows::custom::log_signup::run(...).await?;
    Ok(Json(user))
}
```

Build one `flows::custom::signup` that chains routines.

**Implicit no-retry (omitted Crank):**

```rust
pub async fn run(ctx: &Ctx) -> Result<Vec<UserPublic>, MeltDown> {
    routines::custom::users::list(ctx).await
}
```

Banned. Wrap in `Crank::none()` so the policy is visible:

```rust
pub async fn run(ctx: &Ctx) -> Result<Vec<UserPublic>, MeltDown> {
    Crank::none()
        .run(|| routines::custom::users::list(ctx))
        .await
}
```

**Business logic in transport:**

```rust
pub async fn get_user(State(ctx), Path(id)) -> Result<...> {
    let user = models::users::get(&ctx.conn(), id).await?;
    if user.role == UserRole::Admin {
        ...
    }
}
```

Banned twice over: transport touched models, AND branched on business state. Push into a flow, which calls a routine.

## Related Specs

- `SPEC_ARCHITECTURE.md` — the strict dep graph + Ctx/transaction API
- `SPEC_MELTDOWN.md` — return type
- `SPEC_CRANK.md` — retry combinator inside flows
- `SPEC_RELAY.md` — WS handlers that call flows
- `SPEC_FUSES.md` — scheduled triggers that call flows
- `blast/doc/SPEC_CODEGEN.md` — how Blast generates flows from resource state files
