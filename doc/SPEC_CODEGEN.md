# SPEC_CODEGEN

How Blast generates code. Inputs, outputs, regeneration rules, and the generated/custom boundary.

## Inputs

Blast reads from three sources:

- **`src/database/schema.rs`** — Diesel-emitted schema, regenerated from migrations. **Authoritative source for column names, types, nullability.** Blast parses this file (Diesel's `table!` macro output is stable, so a narrow parser is reliable).
- **`target/primer/*.json`** — per-resource policy IR, produced by the user's `primer/` sub-crate IR emitter. Field *variants* and *validation* only; no types.
- **`target/blueprint/*.json`** — `manifest.json`, `fe_lint.json`. App-level config.

IR is the contract between user's typed Rust config and Blast. Blast does not parse arbitrary Rust source; it reads IR JSON + the narrowly-scoped `schema.rs`.

**Field types always come from `schema.rs`.** If a Primer references a column missing from `schema.rs`, Blast errors out. If `schema.rs` has columns not mentioned in Primer, they're silently skipped — no codegen for them (reachable via `sql_query` / custom models only).

## Outputs (Per Resource)

For a primer declaring `users` with verbs `[list, get, update, delete]` and WS events on `role` changes:

### Rust output

```
src/structs/generated/users.rs
  - User                          (DB row — Queryable)
  - NewUser                       (Insertable)
  - UserPatch                     (AsChangeset, all fields optional)
  - UserPublic                    (Public-variant response shape)
  - UserAdmin                     (Admin-variant response shape)
  - UserEvent                     (WS event enum)
  - impl From<User> for UserPublic
  - impl From<User> for UserAdmin
  - impl From<NewUser> for User (insertion-shape conversion helper)

src/models/generated/users.rs
  - pub async fn list(conn, &Pagination, &UserListFilters) -> Result<Vec<User>, MeltDown>
  - pub async fn get(conn, id) -> Result<User, MeltDown>
  - pub async fn update(conn, id, &UserPatch) -> Result<User, MeltDown>
  - pub async fn delete(conn, id) -> Result<(), MeltDown>
  - (update/delete call relay::publish on success)

src/flows/generated/users/list.rs
src/flows/generated/users/get.rs
src/flows/generated/users/update.rs
src/flows/generated/users/delete.rs
  - Each contains: pub async fn run(ctx, input) -> Result<Output, MeltDown>
  - Auth enforcement per primer hints (auth_required, scoped_to, admin_only)

src/transport/http/generated/users.rs
  - pub fn routes() -> Router
  - Route handlers: list_users, get_user, update_user, delete_user
  - Each handler extracts params + calls corresponding flow + returns typed response

src/transport/ws/generated/users.rs
  - pub fn register_topics(relay: &mut RelayRegistry)
  - Topic type: OrderTopic::Customer { customer_id }
  - WsAuth stub trait impl (user fills in can_subscribe)

src/structs/generated/mod.rs    (re-exports)
src/models/generated/mod.rs     (re-exports)
src/flows/generated/mod.rs      (re-exports)
```

### TS output

```
frontend/src/generated/types/users.ts
  - UserPublic, UserAdmin, UserPatch, NewUser, UserListFilters
  - UserEvent (WS event union)

frontend/src/generated/types/meltdown.ts
  - MeltType const enum (mirrors Rust MeltType variants)
  - MeltDownResponse type

frontend/src/generated/api/users.ts
  - listUsers(pagination, filters) -> Promise<Result>
  - getUser(id) -> Promise<Result>
  - updateUser(id, patch) -> Promise<Result>
  - deleteUser(id) -> Promise<Result>

frontend/src/generated/composables/users.ts
  - useUsers({ poll?, live?, filter? })
  - useUser(id)
  - useUpdateUser()
  - useDeleteUser()

frontend/src/generated/ws/client.ts
  - Shared WsClient singleton
  - .subscribe(topic, handler) / .unsubscribe(topic)
  - Reconnect logic, ping/pong

frontend/src/generated/types/index.ts   (barrel)
frontend/src/generated/api/index.ts     (barrel)
frontend/src/generated/composables/index.ts
```

### TS validator output

```
frontend/src/generated/validators/users.ts
  - validateNewUser(input: unknown): Result<NewUser, ValidationError[]>
  - validateUserPatch(input: unknown): Result<UserPatch, ValidationError[]>
  - validateUserListFilters(input: unknown): Result<UserListFilters, ValidationError[]>
```

Generated from primer validation modifiers (`.max_len()`, `.pattern()`, `.enum_values()`, `.min()`, `.max()`) plus the list endpoint wire schema (`.filtered_by()`, `.paginated()`). Called by generated API clients before the fetch; surfaces errors client-side without a network round-trip.

Rule: every constraint declared in Primer emits both a Rust validator (in the generated route handler's extractor) and a TS validator. The two are structurally mirrored. Blast's `gen frontend` pass drives both in a single codegen cycle to keep them in sync.

### Admin shell route output

```
src/transport/http/generated/admin/users.rs
  - admin::users::routes() -> Router            (mounted under /admin/users)
  - list handler: GET  /admin/users
  - detail handler: GET  /admin/users/:id
  - edit handler: GET  /admin/users/:id/edit
  - update handler: PATCH /admin/users/:id
  - delete handler: DELETE /admin/users/:id
  - action handlers: POST /admin/users/:id/actions/:action_slug  (one per blueprint admin_action)

frontend/src/generated/admin/users.ts
  - AdminUserPublic (Admin-variant shape)
  - admin API client (list, get, update, delete, action calls)
  - useAdminUsers(), useAdminUser(id) composables
```

Generated from: primer (Admin variant field list, admin_hints) + blueprint (admin_actions). The admin shell is a schema-driven generic UI; Blast emits route handlers and typed FE clients for it. The admin list/detail rendering is handled by a shared generic admin shell component — Blast does not emit per-resource Vue pages for admin.

See `catalyst/doc/SPEC_ADMIN.md` for the admin shell layout and the generic component.

### Test scaffold output

Emitted by `blast gen test`. Idempotent — does not overwrite existing files.

```
src/flows/generated/users/list.test.rs        (per declared verb: fixture insert → flow call → assert)
src/flows/generated/users/get.test.rs
src/flows/generated/users/create.test.rs
src/flows/generated/users/update.test.rs
src/flows/generated/users/delete.test.rs

src/transport/http/generated/users.test.rs    (per resource: oneshot request through full stack)

tests/fixtures/users.rs                       (`impl Fixture for User` calling create flow)
tests/fixtures/mod.rs                         (barrel re-exporting every fixture module)
tests/common/mod.rs                           (shared `use catalyst::testing::*` + `test_pool` helper)
```

Each scaffold consumes the catalyst testing harness shipped behind the `testing` Cargo feature:

- `catalyst::testing::with_test_transaction` — always-rollback Postgres wrapper
- `catalyst::testing::run_in_test` — composes the wrapper with a `TestCtxBuilder`
- `catalyst::testing::TestCtx<'a>` — flow-shaped test context (conn + session)
- `catalyst::testing::Fixture` trait + `catalyst::fixture!` macro — flow-driven fixture data

CLI surface: `blast gen test`, `blast gen test --flow <table>` or `<table>/<verb>`, `blast gen test --route <table>`. See `SPEC_BLAST_COMMANDS.md` and `catalyst/doc/SPEC_TESTING.md`.

### Misc output

```
.env.example                               (from blueprint env spec)
frontend/scripts/governor-plugin.js        (Vite plugin wrapper, invokes `blast check`)
frontend/.rule_violations_whitelist        (from blueprint fe_lint.whitelist_snippets)
```

## Generated/Custom Split

**In-layer** subdir convention. Every layer with codegen has both:

```
src/flows/
├── generated/      ← Blast-owned, regenerable
└── custom/         ← user-owned, Blast never touches
```

`mod.rs` re-exports both:

```rust
pub mod generated;
pub mod custom;
```

### Blast's invariants

- `generated/` subtree is **rewritten wholesale** on `blast gen`
- Any hand-edit to a file under `generated/` gets stomped next regen
- `custom/` subtree is **never read, touched, deleted, or renamed** by Blast
- `mod.rs` at each layer re-exports both; Blast regenerates only the generated side

## Regeneration Behavior

`blast gen <target>`:
- `blast gen table [name]` — TUI migration wizard; emits a diesel migration (up.sql / down.sql) in `migrations/`. Does not apply; user runs `blast migrate` after.
- `blast gen migration --custom <name>` — empty migration scaffold for hand-written SQL (views, triggers, partial indexes, etc.)
- `blast gen schema` — runs `diesel print-schema`; writes `src/database/schema.rs` (this is the authoritative type source)
- `blast gen primer [resource]` — compiles `primer/` sub-crate, runs its IR emitter; writes `target/primer/*.json`. Validates all column references against `schema.rs`.
- `blast gen blueprint` — compiles `blueprint/` sub-crate, runs its IR emitter; writes `target/blueprint/*.json`
- `blast gen structs` — reads schema.rs + primer IR; writes `src/structs/generated/`
- `blast gen models` — reads schema.rs + primer IR; writes `src/models/generated/`
- `blast gen flows` — reads primer IR; writes `src/flows/generated/` + `src/transport/http/generated/` + `src/transport/ws/generated/`
- `blast gen frontend` — reads schema.rs + primer IR + blueprint IR; writes `frontend/src/generated/` (types, API clients, composables, TS validators, admin clients)
- `blast gen env-example` — reads blueprint manifest IR; writes `.env.example`
- `blast gen governor-plugin` — reads blueprint fe_lint IR; writes `frontend/scripts/governor-plugin.js` + `.rule_violations_whitelist`
- `blast gen test` — reads primer IR; scaffolds `*.test.rs` per flow + per route; idempotent on existing files
- `blast gen all` — full pipeline: schema → primer → blueprint → structs → models → flows → frontend → env-example → governor-plugin → test scaffolds

**Hard order:** schema must exist before primer IR can be validated. Primer IR must exist before structs/models/flows can generate. Blast's commands check prerequisites and fail loudly if missing.

## Determinism

Generated output is deterministic. Same IR + same Blast version → byte-identical files. This matters for:

- Clean diffs on regen
- Reviewability (generated changes look like sensible diffs, not noise)
- Git hygiene (committed generated files don't flap)

Sorting applied: struct fields in canonical order (PK first, then alphabetical), `mod.rs` re-exports alphabetical, HashMap iteration via BTreeMap for stable output.

## Generation Strategy

Current: `format!()` string templates. Each generator (`structs_gen.rs`, `models_gen.rs`, `flows_gen.rs`, etc.) builds output via nested `format!` calls.

Pros:
- Simple. No AST library. Readable.
- Consistent with existing Blast generators (`structs.rs`, `models.rs`).
- Fast.

Cons:
- Verbose for complex generators.
- No type-safety on output correctness (you can emit broken Rust).
- Escape-your-own-braces pain with `format!` syntax.

Migration path (if it gets painful): `quote!` + `syn` for AST-based generation. Not a blocker v1.

## What Blast Does NOT Parse

- User's Rust source files (models/custom/, flows/custom/, anywhere)
- User's Vue/TS source files (frontend/custom/)
- `Cargo.toml` of user's app (except for minimal name lookup in `blast new`)

Blast reads: IR JSON files + `schema.rs` (Diesel output, considered stable format). Nothing else.

## What Blast OWNS (Writes)

- Everything under `src/*/generated/`
- Everything under `frontend/src/generated/`
- `.env.example`
- `frontend/scripts/governor-plugin.js`
- `frontend/.rule_violations_whitelist`
- `src/database/schema.rs` (indirectly, by invoking Diesel CLI)
- `flows/generated/**/*.test.rs` (initial scaffold only; not overwritten once written)
- `transport/http/generated/**/*.test.rs` (initial scaffold only)
- `tests/fixtures/<resource>.rs` (initial scaffold only)

## What Blast DOES NOT Write

- Anything under `custom/` subdirs
- User's hand-written Vue SFCs / pages
- `Cargo.toml` (after `blast new` initial scaffold)
- Migrations (user writes them; Blast only runs them)
- `.env` (that's secrets + runtime values)
- Primer and Blueprint source files (user authors these; Blast's TUI scaffolds on `blast gen primer <new_resource>`)

## TUI-Driven Generation

`blast gen` with no args launches TUI (dialoguer). User picks what to generate. For `blast gen primer <resource>` specifically, TUI walks through field variants, flow verbs, auth, and WS events — producing a `primer/src/<resource>.rs` file. See `SPEC_BLAST_COMMANDS.md`.

## Anti-Patterns (for Blast maintainers)

- Generating code that references non-generated paths. If user hasn't written `flows/custom/signup.rs`, don't emit an import for it.
- Overwriting `mod.rs` that mixes generated + custom. Keep `mod.rs` files at the layer boundary simple: `pub mod generated; pub mod custom;` — never list individual files.
- Emitting unstable output (iteration over HashMap without sorting). Always sort for determinism.
- Assuming migration has been run. Always `blast gen schema` first, error if `schema.rs` is missing.
- Emitting to `custom/` subdirs. Ever. Blast's generators must refuse to write there.

## Related Specs

- `catalyst/doc/SPEC_PRIMER.md` — primer IR contents
- `catalyst/doc/SPEC_BLUEPRINT.md` — blueprint IR contents
- `catalyst/doc/SPEC_ARCHITECTURE.md` — where generated files land
- `SPEC_GOVERNOR.md` — FE lint
- `SPEC_BLAST_COMMANDS.md` — CLI surface
