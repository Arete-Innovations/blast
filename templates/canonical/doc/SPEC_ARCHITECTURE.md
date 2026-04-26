# SPEC_ARCHITECTURE

Strict, compiler-enforced layered architecture. Dep direction is law.

## Layer Graph

```
structs/       ← data definitions
database/      ← pool, migrations (stateful)
models/        ← persistence (per-resource + cross-resource reads)
services/      ← stateless adapters (no DB, no HTTP)
routines/      ← reusable procedures shared across ≥2 flows
flows/         ← named business operations (capability inventory)
transport/     ← thin external entry points (http / ws / fuses)
frontend/      ← Vue, typed from contracts
```

## Dependency Direction (Law)

| Layer | Can import from |
|-------|-----------------|
| `structs/` | (none) |
| `database/` | `structs` |
| `models/` | `structs`, `database` |
| `services/` | `structs` |
| `routines/` | `structs`, `models`, `services`, `database` |
| `flows/` | `structs`, `models`, `services`, `routines`, `database` |
| `transport/` | **`structs`, `flows` ONLY** — cannot import routines/models/services |
| `frontend/` | Generated TS + hand-written Vue |

**Transport's constraint is the strictest.** It physically cannot bypass flows to reach models. Enforced via Cargo workspace crate boundaries or `mod` visibility where feasible.

## Layer Semantics

### `structs/`

- Inert data definitions: DB row shapes, DTOs, request/response structs, newtypes
- No business logic beyond trait impls and trivial conversions
- Subdirs: `generated/` (Blast emits) + `custom/` (hand-written)

### `database/`

- Connection pool, pool accessor
- Migration runner, bootstrap helpers
- Diesel schema file (`schema.rs`) regenerated from migrations
- **Stateful layer** — holds the pool

### `models/`

- Per-resource CRUD (`generated/{resource}.rs`)
- Cross-resource reads, report aggregates, raw `sql_query` (`custom/*.rs`)
- Functions take `&mut Connection` or `&PgPool` as parameters
- No HTTP, no business flow, no frontend logic
- Stateless — receives pool, returns Results

### `services/`

- Stateless no-DB adapters
- Subdirs by capability: `crypto/`, `email/`, `storage/`, `external/` (etc.)
- One public fn per file where possible; ugly private helpers underneath are fine
- Common pattern: take inputs, return Result, single-shot attempt (retry is for flows, not services)
- **No DB access.** If a service needs data, caller reads it and passes in.

### `routines/`

- Mid-level glue procedures **shared across 2+ flows**
- Subdirs by intent: `act/` (mutations), `collect/` (reads), `derive/` (pure transforms)
- Stateless — accept all dependencies as params
- Sequential, `?`-heavy, readable
- **Do NOT own retry policy** (flows own retries). Routines may expose reusable retry helpers.
- Reuse threshold: used once = inline in the flow file (private fn). Promote to `routines/` only on second use.

### `flows/`

- **Named business operations. The app's capability inventory.**
- One file per named op (`flows/custom/signup.rs`, `flows/generated/users/list.rs`).
- Trivial flows are legal — a flow wrapping one model call is still the canonical entry point for that operation.
- Subdirs: `generated/` (Blast emits for declared CRUD verbs) + `custom/` (user-written business ops).
- **Custom flows compose via models/routines/services directly.** They do NOT wrap generated flows.
- Flow owns retry policy for its operation.
- Flow owns auth enforcement.
- Elegance gradient peaks here: flows should read close to English.

### `transport/`

- Thin external entry points. Three sub-layers:
  - `transport/http/` — Axum route handlers
  - `transport/ws/` — WebSocket handlers (stateful per-connection)
  - `transport/fuses/` — scheduled task runners (stateful, long-running loops)
- Each transport handler extracts+validates input, calls **exactly one flow**, maps the Result to a response.
- Transport cannot import `routines`, `models`, `services`. Period.
- `transport/http/generated/` + `transport/http/custom/`. Same for WS and fuses.

### `frontend/`

- Vue 3 + TypeScript + Vite + PrimeVue
- `generated/` — types, API clients, composables emitted by Blast from contracts
- `custom/` — hand-written Vue SFCs, pages, composables

## Hard Rules

### Transport calls flows only

One HTTP route handler → one flow call. Done. No branching business logic in routes. No multi-flow sequencing in transport.

### Flow == named operation, not "glue"

Don't worry if a flow is thin. `flows/custom/list_users.rs = models::users::list(conn).await.map(Into::into)` is a legitimate flow. Its value is being the named operation in the capability inventory, not its complexity.

### Routines exist for ≥2-flow reuse

Used once → inline. Promote to `routines/` only when shared. Keeps routine namespace meaningful.

### Retries live in flows

The flow owns retry policy for its operation. Services do single-shot attempts. Routines may expose reusable retry helpers (e.g. `routines::infra::with_db_retry`) but don't decide the policy.

See `SPEC_CRANK.md` for the retry combinator.

### State rule

Long-lived mutable state is confined to:
- `database/` (pool)
- `transport/ws/` (per-connection state)
- `transport/fuses/` (long-running loops)

Everything else: pure functions. All dependencies passed as parameters. No implicit global state.

### Error rule

Single app-wide `MeltDown` error enum (`thiserror` + `#[from]`). `IntoResponse` impl lives only in `transport/http/`. Every flow returns `Result<T, MeltDown>`.

See `SPEC_MELTDOWN.md`.

### Ugliness gradient

```
flows/      ← elegant, English-readable, 5-30 lines typical
routines/   ← sequential, ?-heavy, 30-100 lines typical
services/   ← can have ugly private helpers, one public fn per file
models/     ← SQL-heavy, can be ugly
```

The higher in the stack, the more readable the code. The lower, the more implementation-detail work hides. Reviewable rule.

### Generated vs custom split is in-layer

Every layer with codegen has `layer/generated/` and `layer/custom/` subdirs. `mod.rs` re-exports both.

- Blast NEVER touches `custom/`
- Hand-editing `generated/` is a footgun — changes will be overwritten on regen
- User flows, routes, SFCs go in `custom/`

## Example: user signup with welcome email

This demonstrates the composition pattern.

```rust
// flows/custom/signup.rs
use catalyst::meltdown::*;
use catalyst::prelude::*;
use crate::{models, services, structs::users::*};

pub async fn run(ctx: &Ctx, input: SignupInput) -> Result<UserPublic, MeltDown> {
    let password_hash = services::crypto::hash_password(&input.password)?;
    let insertable = NewUser {
        email: input.email,
        password_hash,
        role: UserRole::Member,
    };
    let user = models::users::create(ctx.conn(), &insertable).await?;
    services::email::send_welcome(&user.email).await?;
    Ok(user.into_public())
}
```

```rust
// transport/http/custom/signup.rs
pub async fn signup(
    State(ctx): State<Ctx>,
    Json(input): Json<SignupInput>,
) -> Result<Json<UserPublic>, MeltDown> {
    let user = flows::custom::signup::run(&ctx, input).await?;
    Ok(Json(user))
}
```

The flow composes models + services directly. No orchestrator-on-top-of-routine nonsense. Route is thin.

## What This Replaces

Earlier spec (in legacy `/catalyst/AGENTS.md`) had:

```
structs → database → models → services → routines → orchestrators → routes → frontend
```

With "orchestrators" as a distinct middle layer. Empirical review of the ophanim and upnumbers layer experiments showed ~40-80% of orchestrators were thin wrappers adding indirection with no value. That layer is reframed here as `flows` — no "glue" semantics, just "named operation." Transport cannot bypass to inner layers, which recovers the strictness that the orchestrator layer was meant to provide.
