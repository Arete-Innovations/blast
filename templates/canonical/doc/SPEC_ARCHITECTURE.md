# SPEC_ARCHITECTURE

Strict, compiler-enforced layered architecture. Calls go strictly downward. The chain: `transport → flow → routine → {models, services, database}`.

## Layer Graph

```
structs/       ← inert data definitions
database/      ← pool, migrations (stateful)
services/      ← stateless adapters (no DB, no HTTP)
models/        ← persistence (per-resource + cross-resource reads)
routines/      ← atomic capabilities (compose models + services within ONE op)
flows/         ← capability inventory: routine(s) under a Crank policy + auth boundary
transport/     ← thin external entry points (http / ws / fuses)
frontend/      ← Vue, typed from contracts
```

## Dependency Direction (Law)

| Layer | Can import from |
|-------|-----------------|
| `structs/` | (none) |
| `database/` | `structs` |
| `services/` | `structs` |
| `models/` | `structs`, `database` |
| `routines/` | `structs`, `models`, `services` |
| `flows/` | **`structs`, `routines`, `crank` ONLY** |
| `transport/` | **`structs`, `flows` ONLY** |
| `frontend/` | generated TS + custom Vue |

The strictness is enforced at compile time by `build.rs`, which scans every `use crate::*` import under `src/<layer>/` (single-line and multi-line `use crate::{...}` blocks) and panics the build on any forbidden cross-layer import. See `LAYER:11`–`LAYER:17` in `build.rs`. The lint reads each file's first path segment under `src/` to determine its layer, then checks every flattened import path against that layer's banned-prefix list. Aliases (`as`), glob (`*`), self-imports, and arbitrarily nested groups are all flattened before checking.

## Hard Rules

### Transport calls flows only

One HTTP / WS / Fuses handler → one flow call. No branching business logic. No multi-flow sequencing. Auth-or-no-auth gating is at transport middleware.

### Flows call routines only

Every flow body is a composition of routine calls under a `Crank` retry policy. No `models::*`, no `services::*`, no `database::*` allowed in `flows/`.

### Flows do not call flows

`flows/foo` MAY NOT call `flows/bar`. Shared logic is a routine, not a flow.

### Routines are leaves

A routine composes models, services, and database access for ONE atomic capability. Routines do NOT call other routines. If composition of routines is needed, that's a flow.

### Every flow declares a Crank policy

Even when no retry is desired, a flow declares `Crank::none()` explicitly. Retry policy is a first-class, audit-able fact per capability. The policy IS part of the flow's contract — `Arsenal` and operators read it directly off the flow source.

### Flows are the capability inventory

The set of files in `flows/` is the answer to "what can this app do?". `Arsenal` walks `flows/` to emit the capability listing.

## Layer Semantics

### `Ctx` (universal handle, not a layer)

`Ctx` is the request-scoped handle threaded into every layer above `database/`. Lives at crate root (`src/ctx.rs`). Owns the pool reference and the session state. Exposes:

- `ctx.conn()` — acquire a pooled connection (used by routines, passed to models)
- `ctx.transaction(|tx| async move { ... })` — atomic multi-model wrapper. Auto-commits on `Ok`, auto-rolls-back on `Err`. **Routines only.** Returns `Result<T, MeltDown>`.
- `ctx.require_anonymous()`, `ctx.require_role(...)` — auth gates (used by flows)

`Ctx` is not a layer; it's a parameter type. Its presence in a fn signature does not break the dep graph.

### `structs/`

Inert data definitions: DB rows, DTOs, request/response, newtypes. Trait impls and trivial conversions only. Subdirs: `generated/` (Blast-owned) + top-level area subdirs (user-owned: `auth/`, `sessions/`, `<your_resource>/`, etc.).

**Schema-import exception:** structs may import `crate::database::schema` (the Diesel `table!` macro output) for `#[diesel(table_name = ...)]` derives on `Queryable` / `Insertable` row structs. The schema module is generated macro definitions, not a layer dependency. This is the *only* `crate::*` import permitted in `structs/`. The build.rs lint encodes this exception explicitly.

### `database/`

Connection pool + accessor, migration runner, Diesel `schema.rs`. Stateful — holds the pool.

### `services/`

Stateless, no-DB adapters. Subdirs by capability (`crypto/`, `email/`, `storage/`, `external/`). One public fn per file. **Single-shot** — no retry policy here. If a service needs data, the caller passes it in.

### `models/`

Per-resource CRUD and cross-resource reads. Functions take `&mut Connection` or `&PgPool`. SQL-heavy. Stateless.

### `routines/`

Atomic capabilities. Each routine is one named operation that may compose multiple model + service calls internally — but constitutes ONE business action. **Cannot call other routines. Cannot import `database` directly** — if a DB op is needed, the corresponding model must expose it. Routines acquire a connection via `&Ctx` and hand it to a model; they never see Diesel, never see the pool primitive.

When a routine needs ≥2 model calls to be atomic, it wraps them in `ctx.transaction(|tx| async move { ... })` and hands `tx` to each model in place of `ctx.conn()`. Single-call routines need no wrapper — Postgres autocommits each statement.

Generated/user split: `generated/` (Blast emits CRUD leaves, regenerable) + top-level resource subdirs (user-owned, hand-written).

Org convention: by resource (`routines/users/...`, `routines/auth/...`) — one project-wide rule. The canonical template ships `auth/` and `sessions/` pre-populated at scaffold time; once scaffolded, those files are yours to edit.

### `flows/`

The capability inventory. Each flow is:

- one or more routine calls,
- each wrapped in a `Crank` policy (use `Crank::none()` if no retry),
- preceded by an auth/role check on `&Ctx`,
- returning `Result<T, MeltDown>`.

Flow body is formulaic by design. The value is registration + policy + auth, not orchestration complexity.

Subdirs: `generated/` (Blast emits — typically `Crank::none()` around a single generated routine) + top-level resource subdirs (user-owned, multi-routine business ops).

### `transport/`

Thin external entry points. Three sub-layers:

- `transport/http/` — Axum route handlers
- `transport/ws/` — WebSocket handlers (stateful per-connection)
- `transport/fuses/` — scheduled task runners (stateful, long-running)

Each handler: extract input → call exactly one flow → map `Result` to response. Cannot import routines / models / services / database.

### `frontend/`

Vue 3 + TS + Vite + PrimeVue. `generated/` + `custom/` split inside `frontend/src/`.

## Routine vs Flow Heuristic

| Question | Routine | Flow |
|----------|---------|------|
| Atomic single-purpose? | yes | no |
| Multiple steps that may need independent retry? | no | yes |
| Has a retry policy? | no — single-shot | yes — even if `Crank::none()` |
| Listed in the capability inventory? | no | yes |
| Callable from transport? | no | yes |

If you want to call a routine from another routine, you actually want a flow.

## Example: signup with welcome email

```rust
pub async fn create(ctx: &Ctx, input: &SignupInput) -> Result<User, MeltDown> {
    let hash = services::crypto::hash_password(&input.password)?;
    let new = NewUser {
        email: input.email.clone(),
        password_hash: hash,
        role: UserRole::Member,
    };
    models::users::create(ctx.conn(), &new).await
}
```

```rust
pub async fn send(addr: &str) -> Result<(), MeltDown> {
    services::email::send_welcome(addr).await
}
```

```rust
pub async fn run(ctx: &Ctx, input: SignupInput) -> Result<UserPublic, MeltDown> {
    ctx.require_anonymous()?;
    let user = Crank::none()
        .run(|| routines::users::create(ctx, &input))
        .await?;
    Crank::backoff(3, Duration::from_millis(500))
        .run(|| routines::email::welcome::send(&user.email))
        .await?;
    Ok(user.into_public())
}
```

```rust
pub async fn signup(
    State(ctx): State<Ctx>,
    Json(input): Json<SignupInput>,
) -> Result<Json<UserPublic>, MeltDown> {
    let user = flows::signup::run(&ctx, input).await?;
    Ok(Json(user))
}
```

The flow declares two distinct retry policies: zero retries on user creation, three on email send. Routines stay pure capability units, reusable across other flows.

## Generated vs User-owned split (two tiers, flat)

Every layer with codegen has a `generated/` subdir alongside top-level resource subdirs.

| Bucket | Owner | Behavior |
|--------|-------|----------|
| `<layer>/generated/` | Blast | rewritten wholesale on `blast gen`; never hand-edit |
| `<layer>/<resource>/` (top-level subdirs) | User | Blast never reads, touches, deletes, or renames |

The canonical template ships `flows/auth/`, `flows/sessions/`, `routines/auth/`, `routines/sessions/`, `models/auth/`, `structs/auth/`, `structs/sessions/`, `transport/http/auth.rs`, `transport/http/healthz.rs`, etc. Once scaffolded, these files are user-owned — modify freely, Blast never touches them again. Framework upgrades come via `git diff` against upstream `canonical/`, not via a `vendor-update` command.

For default CRUD per resource, Blast emits:
- one routine per verb in `routines/generated/<r>/{list,get,create,update,delete}.rs`
- one flow per verb in `flows/generated/<r>/...` — each flow is `Crank::none()` around the matching routine, with the resource's declared auth check
- one route per verb in `transport/http/generated/<r>/...`

User-written multi-step business ops are flows in `flows/<resource>/`. Hand-editing `generated/` is a footgun — overwritten on next `blast gen`.

## State rule

Long-lived mutable state confined to:

- `database/` (pool)
- `transport/ws/` (per-connection state)
- `transport/fuses/` (long-running loops)

Everything else: pure functions, deps as parameters.

## Error rule

Single app-wide `MeltDown` error enum (`thiserror` + `#[from]`). `IntoResponse` impl lives only in `transport/http/`. Every routine and every flow returns `Result<T, MeltDown>`. See `SPEC_MELTDOWN.md`.

## Ugliness gradient

```
flows/      ← formulaic, English-readable, 5-20 lines per flow
routines/   ← sequential, ?-heavy, 20-80 lines per routine
services/   ← can have ugly private helpers, one public fn per file
models/     ← SQL-heavy, can be ugly
```

The higher in the stack, the more readable the code. The lower, the more impl detail hides.
