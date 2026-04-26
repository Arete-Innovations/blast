# SPEC_CODEGEN

How Blast generates code. Inputs, outputs, state-hash markers, regeneration rules, and the generated/custom boundary.

## Inputs

Blast reads from two sources:

- **`src/database/schema.rs`** — Diesel-emitted schema, regenerated from migrations. **Authoritative source for column names, types, nullability.** Blast parses this file (Diesel's `table!` macro output is stable, so a narrow parser is reliable).
- **`storage/blast/state/`** — RON state files authored by the TUI wizard or by hand. `app.ron` for app-wide policy; `resources/<name>.ron` per resource.

Blast does not read `target/primer/*.json`, `target/blueprint/*.json`, or any compiled Rust sub-crate. The DSL crates (`catalyst_primer`, `catalyst_blueprint`) are deleted. State files are the single source of truth for policy.

Resource state loading is centralized in `crate::codegen::ir_loader::load_resource_states(project_root)` — every codegen pass uses it, so they all see the same set of resources in the same order (alphabetical on table name).

**Field types always come from `schema.rs`.** If a resource state file references a column missing from `schema.rs`, Blast errors out. If `schema.rs` has columns not mentioned in a resource state file, they're silently skipped — reachable via `sql_query` / custom models only.

See `SPEC_STATE.md` for the full state file format and schema versioning rules.

## State-Hash Markers

Every generated file opens with a marker comment carrying the **blake3** content hash of the state file it was produced from. The marker is emitted by the centralized `crate::codegen::header` module — every codegen pass calls `header::marker_for_resource(...)` / `header::marker_for_app(...)` / `header::marker_for_schema(...)` and prepends the result to the file body.

Format (byte-stable, no timestamps, no clocks):

```rust
// AUTO-GENERATED from storage/blast/state/resources/users.ron @ <blake3-hex>
//
// Do not edit by hand. Run `blast gen all` after mutating state.
```

Three convenience helpers cover the three sources:

| Helper | State file | Used by |
|--------|------------|---------|
| `header::marker_for_resource(root, table)` | `storage/blast/state/resources/<table>.ron` | structs, flows, http_routes, ws_topics, vue, frontend types/api/composables |
| `header::marker_for_app(root)`              | `storage/blast/state/app.ron` | env_example, governor_plugin, barrels (`mod.rs` re-exports), root vue index |
| `header::marker_for_schema(root)`           | `src/database/schema.rs` | (reserved for the upcoming models v2 codegen) |

The marker is parsed back at compile time by the user app's `build.rs` (template at `crate::codegen::build_rs_template`, see below). On hash mismatch `build.rs` calls `panic!` so `cargo check` / `cargo build` / `cargo test` all hard-fail with an actionable message.

Users cannot forget to regen. Stale codegen is a compile error.

## Outputs (Per Resource)

For a resource state file declaring `users` with verbs `[list, get, update, delete]` and WS events on `role` changes:

### Rust output

Naming convention for projection structs (locked, no exceptions):

```
<TypeStem><Variant>
```

For a resource named `users`:

| Variant | Type name | Role |
|---------|-----------|------|
| `Db` | `User` | Diesel `Queryable` row — backs every other projection |
| `Insertable` | `UserInsertable` | Diesel `Insertable` for `create` flow |
| `Patch` | `UserPatch` | Diesel `AsChangeset`, all fields `Option<T>` |
| `Public` | `UserPublic` | Response shape returned to authenticated users |
| `Admin` | `UserAdmin` | Response shape returned through admin-only routes |
| (filter) | `UserFilter` | Query shape for List endpoint (derived from `filterable_columns`) |

No `ForCreate` / `ForUpdate` / `Row` / `New` suffix sprawl. The variant *is* the suffix, full stop. Implementation in `src/codegen/structs/{mod, runner, naming, sql_map, emitter}.rs`.

```
src/structs/generated/users.rs
  - User                          (Db row — Queryable)
  - UserInsertable                (Insertable)
  - UserPatch                     (AsChangeset, all fields optional)
  - UserPublic                    (Public-variant response shape)
  - UserAdmin                     (Admin-variant response shape)
  - UserFilter                    (List endpoint query shape, when verbs include List with .filtered_by)

src/structs/generated/mod.rs     (barrel — alphabetical re-exports)

src/models/generated/users.rs    (legacy schema-only generator — slated for v2 rewrite)
  - pub async fn list(conn, ...) -> Result<Vec<User>, MeltDown>
  - pub async fn get(conn, id) -> Result<User, MeltDown>
  - pub async fn update(conn, id, &UserPatch) -> Result<User, MeltDown>
  - pub async fn delete(conn, id) -> Result<(), MeltDown>

src/flows/generated/users/list.rs
src/flows/generated/users/get.rs
src/flows/generated/users/create.rs
src/flows/generated/users/update.rs
src/flows/generated/users/delete.rs
  - Each contains: pub async fn run(ctx: &Ctx, input: ...) -> Result<Output, MeltDown>
  - Auth enforcement is dispatched through `Ctx::require_admin()` / `Ctx::require_roles(&[...])`
    per resource state hints (auth_required, scoped_to, admin_only, roles).

src/transport/http/generated/users.rs
  - pub fn router() -> Router
  - Route handlers: list_users, get_user, create_user, update_user, delete_user
  - List handlers extract `catalyst::transport::http::list_query::ListQuery`
    and return `catalyst::transport::http::list_query::ListResponse<UserPublic>`.
  - Each handler calls one flow only.

src/transport/ws/generated/users.rs
  - pub fn register_topics(relay: &mut catalyst::relay::Registry)
  - Topic enum derived from resource state ws_events
  - WsAuth stub trait impl (user fills in can_subscribe)

src/flows/generated/mod.rs      (barrel)
src/transport/http/generated/mod.rs
src/transport/ws/generated/mod.rs
```

`flows`, `http_routes`, `ws_topics` are emitted by **separate** Wave-3 codegen passes (`src/codegen/{flows.rs, http_routes.rs, ws_topics.rs}`), each invoked as its own step in `blast gen all`.

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

Generated from resource state validation modifiers (`.max_len`, `.pattern`, `.enum_values`, `.min`, `.max`) plus the list endpoint wire schema (`.filtered_by`, `.paginated`). Called by generated API clients before the fetch; surfaces errors client-side without a network round-trip.

Rule: every constraint declared in resource state emits both a Rust validator (in the generated route handler's extractor) and a TS validator. The two are structurally mirrored. Blast's `gen frontend` pass drives both in a single codegen cycle to keep them in sync.

### Admin shell route output

```
src/transport/http/generated/admin/users.rs
  - admin::users::routes() -> Router            (mounted under /admin/users)
  - list handler: GET  /admin/users
  - detail handler: GET  /admin/users/:id
  - edit handler: GET  /admin/users/:id/edit
  - update handler: PATCH /admin/users/:id
  - delete handler: DELETE /admin/users/:id
  - action handlers: POST /admin/users/:id/actions/:action_slug  (one per app.ron admin_action)

frontend/src/generated/admin/users.ts
  - AdminUserPublic (Admin-variant shape)
  - admin API client (list, get, update, delete, action calls)
  - useAdminUsers(), useAdminUser(id) composables
```

Generated from: resource state (Admin variant field list) + `app.ron` (admin_actions). See `catalyst/doc/SPEC_ADMIN.md`.

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

### Vue component output

`codegen::vue` emits per-resource Vue SFCs into the user app's `frontend/src/components/<resource>/`:

```
frontend/src/components/users/Form.vue       (when verbs include create or update)
frontend/src/components/users/List.vue       (when verbs include list)
frontend/src/components/users/index.ts       (per-resource barrel)
frontend/src/components/index.ts             (root barrel — alphabetical re-exports)
```

Implementation in `src/codegen/vue/{mod, runner, plan, form, list, barrels, marker, naming, sql_map, report}.rs`. Vue SFCs use HTML-comment markers (`<!-- AUTO-GENERATED ... -->`); TS files use `// AUTO-GENERATED ...` markers. Both forms hash-fail through the same build.rs check.

### Misc output

```
.env.example                               (from app.ron env spec section)
frontend/scripts/governor-plugin.js        (Vite plugin wrapper, invokes `blast check`)
frontend/.rule_violations_whitelist        (from app.ron fe_lint.whitelist_snippets)
build.rs                                   (one-time emit by `blast new` from build_rs_template)
```

The user-app `build.rs` is **not** regenerated by `blast gen all` — it's emitted once by `blast new` (template at `src/codegen/build_rs_template_src.rs.tmpl`, runner at `src/codegen/build_rs_template.rs`) and committed. It is intentionally short, has no external deps beyond `blake3`, and walks `src/{structs,models,flows,transport/http,transport/ws}/generated/` looking for `// AUTO-GENERATED from ...` markers.

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

## Rename Detection and Refusal

When the user renames a resource (e.g. `User` → `Account`) via the TUI wizard, Blast greps `src/**/custom/` for the old symbol before writing the updated state file. If old symbols are found, Blast refuses (or loudly warns with file:line context) until the user resolves them manually. There is no magic AST patching — the layer split is intentional.

## Regeneration Behavior

`blast gen <target>`:
- `blast gen table [name]` — TUI migration wizard; emits a diesel migration (up.sql / down.sql) in `migrations/`. Does not apply; user runs `blast migrate` after.
- `blast gen migration [--custom] <name>` — empty migration scaffold (`--custom` = hand-written SQL: views/triggers/partial indexes/etc.)
- `blast gen schema` — runs `diesel print-schema`; writes `src/database/schema.rs`
- `blast gen resource [name]` — TUI wizard; writes/updates `storage/blast/state/resources/<name>.ron`. Does NOT run codegen.
- `blast gen structs` — reads schema.rs + resource state files; writes `src/structs/generated/`
- `blast gen models` — reads schema.rs + resource state files; writes `src/models/generated/` (legacy generator slated for v2 rewrite)
- `blast gen flows` — reads resource state files; writes `src/flows/generated/`
- `blast gen frontend` — reads schema.rs + resource state files + app.ron; writes `frontend/src/generated/` (types, API clients, composables, TS validators, admin clients)
- `blast gen env-example` — reads app.ron env spec; writes `.env.example`
- `blast gen governor-plugin` — reads app.ron fe_lint section; writes `frontend/scripts/governor-plugin.js` + `.rule_violations_whitelist`
- `blast gen fe-scaffold` — first-run seed: writes `frontend/src/styles/{tokens.css, base.css}` and `frontend/src/plugins/primevue.ts` if absent (idempotent)
- `blast gen test [--flow|--route]` — reads resource state files; scaffolds `*.test.rs` per flow + per route; idempotent on existing files
- `blast gen all` — full eleven-step pipeline; see `SPEC_BLAST_COMMANDS.md` for the exact step list

**Hard order:** `schema.rs` must exist before resource state can be validated (column references checked). Resource state must exist before structs/models/flows can generate. `blast gen all` enforces this implicitly via step ordering; individual targets fail loudly if their prerequisites are missing.

## Determinism

Generated output is **byte-identical for byte-identical state input** across runs, machines, and Blast versions. Same state files + same Blast version → identical files. This matters for:

- Clean diffs on regen
- Reviewability (generated changes look like sensible diffs, not noise)
- Git hygiene (committed generated files don't flap)
- State-hash marker integrity

Rules enforced by all generators:
- Use `BTreeMap` everywhere iteration order matters; never `HashMap`.
- Sort before emit: struct fields in canonical order (PK first, then alphabetical), `mod.rs` re-exports alphabetical, layer entries alphabetical.
- No clocks in codegen output (no `generated_at` timestamps in generated Rust/TS files — only in `arsenal.json`).
- No env vars in codegen logic.
- No random values.

## Generation Strategy

Current: `format!()` string templates. Each generator (`structs_gen.rs`, `models_gen.rs`, `flows_gen.rs`, etc.) builds output via nested `format!` calls.

Pros:
- Simple. No AST library. Readable.
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

Blast reads: `storage/blast/state/*.ron` + `resources/*.ron` + `schema.rs` (Diesel output, considered stable format). Nothing else.

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
- `storage/blast/state/` files except via TUI wizard — never silently rewritten during codegen

## TUI-Driven Generation

`blast gen` with no args launches TUI (dialoguer). User picks what to generate. For `blast gen resource [name]` specifically, TUI walks through field variants, flow verbs, auth, and WS events — producing a `storage/blast/state/resources/<name>.ron` file. The wizard calls the same command core `run` fn as the CLI; it only resolves args, never executes directly. See `SPEC_BLAST_COMMANDS.md`.

## Anti-Patterns (for Blast maintainers)

- Generating code that references non-generated paths. If user hasn't written `flows/custom/signup.rs`, don't emit an import for it.
- Overwriting `mod.rs` that mixes generated + custom. Keep `mod.rs` files at the layer boundary simple: `pub mod generated; pub mod custom;` — never list individual files.
- Emitting unstable output (iteration over HashMap without sorting). Always sort for determinism.
- Assuming migration has been run. Always `blast gen schema` first, error if `schema.rs` is missing.
- Emitting to `custom/` subdirs. Ever. Blast's generators must refuse to write there.
- Reading `target/primer/` or `target/blueprint/` — those paths are gone. Read `storage/blast/state/`.
- Emitting codegen without a state-hash marker in the file header.
- Timestamps, random seeds, or env-var reads inside generator logic.

## Related Specs

- `SPEC_STATE.md` — state file format, schema versioning, atomic write, upgrader contract
- `SPEC_GOVERNOR.md` — FE lint, Vite plugin wrapper emission
- `SPEC_BLAST_COMMANDS.md` — CLI surface
- `catalyst/doc/SPEC_ARCHITECTURE.md` — where generated files land
