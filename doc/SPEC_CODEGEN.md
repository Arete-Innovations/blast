# SPEC_CODEGEN

How Blast generates code. Inputs, outputs, state-hash markers, regeneration rules, and the generated/custom boundary.

## Source-of-truth Model

Apps DO NOT depend on `catalyst` as a Cargo dep. There is no `catalyst = { path = ... }` or `catalyst = { git = ... }` line anywhere. The framework source tree lives in `blast/templates/canonical/` and gets baked into the blast binary at compile time via `include_dir!()`. When `blast new` scaffolds a project it walks that baked tree, substitutes `{{project_name}}` placeholders, and writes a complete framework checkout to the project root. Every scaffolded app is its own fork-by-default copy of the framework.

`templates/canonical/` is the single source of truth. The published `catalyst/` repo is an OUTPUT artifact regenerated from `blast new` at publish time; never edit it by hand.

**Update model (end-user-time):** there is no `vendor-update` command. Framework upgrades are user-driven via `git diff` against upstream `blast/templates/canonical/` — user merges what they want into their fork. Each scaffolded app is a complete framework checkout, and edits stick on the user's checkout indefinitely.

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
| `header::marker_for_app(root)`              | `storage/blast/state/app.ron` | env_example, governor_plugin, barrels (`mod.rs` re-exports), root vue index, theme codegen (`tokens.css`, `primevue.ts`), icons codegen (`icons.ts`) |
| `header::marker_for_schema(root)`           | `src/database/schema.rs` | (reserved for the upcoming models v2 codegen) |

The marker is parsed back at compile time by the user app's `build.rs` (template at `crate::codegen::build_rs_template`, see below). On hash mismatch `build.rs` calls `panic!` so `cargo check` / `cargo build` / `cargo test` all hard-fail with an actionable message.

Users cannot forget to regen. Stale codegen is a compile error.

## Generation Level (per-resource cut-off)

Each resource declares a `gen_level` in its RON state file controlling how far codegen propagates down the pipeline. Levels are linear and monotonic — picking level N implies all levels < N. The wizard offers a single dropdown; power-users can hand-edit RON.

```ron
(
    name: "users",
    fields: [...],
    verbs: [List, Get, Create, Update, Delete],
    gen_level: Composables,  // default
)
```

**Levels (lowest → highest, each implies prior):**

| Level | Adds | Use case |
|-------|------|----------|
| `Struct` | `structs/generated/<r>.rs` (User, UserInsertable, UserPatch, UserPublic, UserAdmin, UserFilter — Structs v2 projections) | data shape only; user writes everything else by hand |
| `Model` | `models/generated/<r>.rs` (Diesel CRUD + cross-resource read helpers) | persistence layer; user owns transport |
| `Route` | `routines/generated/<r>/{list,get,create,update,delete}.rs` + `flows/generated/<r>/...` (Crank::none()) + `transport/http/generated/<r>/...` | full BE CRUD via HTTP; no FE help |
| `Types` | `frontend/src/types/generated/<r>.ts` (TS interfaces mirroring Rust DTOs) + `frontend/src/api/generated/<r>.ts` (typed fetch wrappers) | BE + TS-typed API client; user writes own UI |
| `Composables` | `frontend/src/generated/composables/<r>.ts` (`useUsersList`, `useUser`, `useCreateUser`, `useUpdateUser`, `useDeleteUser`) + `frontend/src/generated/composables/index.ts` (barrel) | reactive Vue logic ready; user writes templates |
| `Components` | `frontend/src/components/generated/forms/<r>/{CreateForm,EditForm}.vue` (PrimeVue form components wired to composables) | user composes own page layouts using generated forms |
| `Pages` | `frontend/src/pages/generated/<r>/{ListPage,DetailPage,CreatePage,EditPage}.vue` + `frontend/src/router/generated/routes.ts` (auto-route table updated) + `frontend/src/nav/generated/menu.ts` (sidebar entry added) | full admin-style CRUD UI shipped — opt-in |

**Default:** `Composables`. Gets typed FE access without committing to UI shape. Power-users opt into `Pages` for "admin-style ship-it" or down to `Struct` for "data only."

**Page generation philosophy:** generated pages target **admin-grade / internal-tool quality**, not production user-facing UI. PrimeVue defaults handle styling. Apps with custom branding are expected to shadow generated pages — write `pages/users/ListPage.vue` at top level, route to it instead of `pages/generated/users/ListPage.vue`. Blast does NOT delete user-shadowed pages; it always rewrites `pages/generated/` wholesale on each `blast gen`.

**Field-type → input-type mapping table** (drives form codegen):

| Rust type | TS input |
|-----------|----------|
| `String` | `<InputText>` |
| `bool` | `<Checkbox>` |
| `i32`, `i64`, `f32`, `f64` | `<InputNumber>` |
| `chrono::DateTime<Utc>` | `<Calendar showTime>` |
| `chrono::NaiveDate` | `<Calendar>` |
| `enum Role` | `<Dropdown :options="...">` |
| FK (e.g. `user_id: i64`) | `<AutoComplete>` against parent resource's list endpoint |
| `Option<T>` | non-required wrapper |

Custom validators emit on both sides — Rust validator in route handler extractor, TS validator in form schema. Structurally mirrored, generated in the same codegen pass to keep them in sync.

**Level-downgrade behavior:** if the user lowers `gen_level` (e.g. `Pages` → `Composables`), the next `blast gen` run STOPS emitting at the new level but does NOT delete files emitted at the previous level. The user must manually delete stale `pages/generated/<r>/` files. Blast warns about orphan generated dirs at higher levels than current `gen_level`.

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

src/models/generated/users.rs    (per-resource model layer — fluent builder + auto-conn impls + module fns)
  - pub async fn list(conn, &ListQuery) -> Result<ListResponse<User>, MeltDown>
  - pub async fn get(conn, id) -> Result<User, MeltDown>
  - pub async fn create(conn, &UserInsertable) -> Result<User, MeltDown>
  - pub async fn update(conn, id, &UserPatch) -> Result<User, MeltDown>
  - pub async fn delete(conn, id) -> Result<(), MeltDown>
  - impl User { auto-conn `pub async fn list/get/create/update/delete` wrappers using `crate::database::acquire_conn()`; `query()`, `count()` shortcuts }
  - pub struct UserQuery { fluent filter builder over `BoxedSelectStatement` }
  - impl IntoFuture for UserQuery / UserQueryPaginated
  - All Diesel calls use `.select(<User as ::diesel::SelectableHelper<::diesel::pg::Pg>>::as_select())` and `.returning(...)` on insert/update so column order is Selectable-driven, not tuple-position-driven.

src/routines/generated/users/list.rs
src/routines/generated/users/get.rs
src/routines/generated/users/create.rs
src/routines/generated/users/update.rs
src/routines/generated/users/delete.rs
  - Each contains: pub async fn run(ctx: &Ctx, args) -> Result<<UserPublic / ListResponse<UserPublic> / ()>, MeltDown>
  - Owns `ctx.conn()`, calls `crate::models::generated::users::<verb>(&mut conn, ...)`, maps Row → Public via `.into()`
    (and `.map(|row| row.into())` for List). Atomic capability — one file per verb. NO auth check (auth lives in flow).

src/routines/generated/mod.rs   (top-level barrel listing each `pub mod <table>;`)
src/routines/generated/users/mod.rs   (per-resource verb barrel)

src/flows/generated/users/list.rs
src/flows/generated/users/get.rs
src/flows/generated/users/create.rs
src/flows/generated/users/update.rs
src/flows/generated/users/delete.rs
  - Each contains: pub async fn run(ctx: &Ctx, args) -> Result<Output, MeltDown>
  - Body is auth check (`ctx.require_session()?` / `require_any(&[Role::Admin, ...])?`) plus
    `Crank::none().run(|| routines::generated::<r>::<verb>::run(ctx, args.clone())).await`
  - AuthMode codegen: Public→none, AuthRequired→require_session, AdminOnly→require_any(&[Role::Admin]),
    Roles([…])→require_any(&[Role::A, Role::B]) (PascalCase Role enum variants), ScopedTo(field)→require_session + TODO scope check.

src/transport/http/generated/users.rs
  - pub fn router() -> Router<Ctx>
  - Route handlers: list, get_one, create, update, delete_one
  - List handlers extract `crate::structs::list_query::ListQuery` (params: ListQuery — uses its own
    FromRequestParts impl, NOT `Query<ListQuery>`) and return
    `Json<crate::structs::list_query::ListResponse<UserPublic>>`.
  - Each handler calls one flow only.

src/transport/ws/generated/users.rs
  - pub fn register_topics(relay: &mut crate::transport::ws::Registry)
  - Topic enum derived from resource state ws_events
  - WsAuth stub trait impl (user fills in can_subscribe)

src/flows/generated/mod.rs      (top-level barrel — `pub mod <table>;` for each emitted resource)
src/transport/http/generated/mod.rs
src/transport/ws/generated/mod.rs
```

`structs`, `models`, `routines`, `flows`, `http_routes`, `ws_topics` are emitted by **separate** codegen passes (`src/codegen/{structs/, models/, routines/, flows.rs, http_routes.rs, ws_topics.rs}`), each invoked as its own step in `blast gen all`. Pipeline order: `schema → enums → structs → models → routines → flows → http_routes → frontend_types → frontend_api → composables → validators → components → pages → theme → icons → env_example → governor_plugin`.

### Enum output (Postgres `CREATE TYPE` → Rust enum + Diesel impls)

When user adds `CREATE TYPE my_status AS ENUM ('a', 'b', 'c');` to a migration, the `enums` codegen pass (`src/codegen/enums/`) detects it and emits a Role-shape Rust file:

```
src/structs/generated/enums/my_status.rs
  - pub enum MyStatus { A, B, C }
  - impl MyStatus { fn as_str() -> &'static str; fn parse(&str) -> Result<Self, MeltDown>; }
  - impl FromSql<MyStatus, Pg> for MyStatus
  - impl ToSql<MyStatus, Pg> for MyStatus
  - sql_type tied to crate::database::schema::sql_types::MyStatus

src/structs/generated/enums/mod.rs   (barrel — `pub mod my_status; pub use my_status::MyStatus;`)
```

`src/codegen/enums/scan.rs::scan_project_enums(project_root) -> ScanReport { enums: Vec<ParsedEnum>, duplicates }` parses `database/migrations/**/*.sql` for `CREATE TYPE ... AS ENUM (...)` declarations. The result is threaded as `&[ParsedEnum]` through downstream FE codegen passes (frontend_types, components, pages) so they can detect enum-typed fields and emit the appropriate TS string-literal-union and Vue Dropdown wiring.

The Diesel `sql_types` marker struct lives in canonical's `src/database/schema.rs` (emitted by `diesel print-schema`) — `STRUCTS:22` build.rs lint exempts that file specifically so the marker can live there.

Reference shape: canonical's hand-rolled `Role` enum at `src/structs/auth/role.rs` is the canonical example every codegen'd enum mirrors. E2E proof: `blast/tests/enum_codegen_e2e.rs` runs the full pipeline against a fixture migration.

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
  - useUsersList(opts?: UseListOpts)            // returns { data, error, loading, refetch, page, pageSize, sort, filter, total, total_pages }
                                                // tier dispatch via opts: {} static, { poll: ms } polled (visibility-pause), { live: true } WS
  - useUser(id: Ref<number>)                    // single-item composable; abort controller threaded; watches id
  - useCreateUser()                             // returns async (input: UserInsertable) => { data?, error? }
  - useUpdateUser()                             // returns async (id, patch: UserPatch) => { data?, error? }
  - useDeleteUser()                             // returns async (id) => { error? }

frontend/src/generated/composables/index.ts     // barrel re-exports per resource
  - export { useUsersList, useUser, useCreateUser, useUpdateUser, useDeleteUser } from './users'

Composables thread URL state via `useUrlListState()` from `@/composables/url` (hand-written
primitive — never re-emitted by Blast). They never own local refs for page/sort/filter
(LocalListState Governor rule). Mutations are non-optimistic — caller awaits the result
and decides what to do with it. Lifecycle: onMounted triggers initial fetch, onUnmounted
aborts in-flight + clears interval + removes WS subscription.

frontend/src/generated/ws/client.ts
  - Shared WsClient singleton
  - .subscribe(topic, handler) / .unsubscribe(topic)
  - Reconnect logic, ping/pong

frontend/src/generated/types/index.ts   (barrel)
frontend/src/generated/api/index.ts     (barrel)
frontend/src/generated/composables/index.ts
```

### Validators output (Rust + TS, single source)

Driven by `FieldState.validators: BTreeSet<ValidatorRule>` in each Primer file. Codegen pass at `src/codegen/validators/` emits paired Rust + TS validators with byte-identical regex strings. See `templates/canonical/doc/SPEC_VALIDATORS.md` for the full rule set + wire-in pattern.

```
src/structs/generated/validators/<r>.rs
  - pub fn validate_<r>_insertable(input: &<R>Insertable) -> Result<(), MeltDown>
  - pub fn validate_<r>_patch(input: &<R>Patch) -> Result<(), MeltDown>
  - lazy_static / once_cell regex constants for any Pattern/Email/Url rules
```

```
frontend/src/generated/validators/<r>.ts
  - export type FieldErrors = Record<string, string>;
  - export function validate<R>Insertable(input: <R>Insertable): FieldErrors | null
  - export function validate<R>Patch(input: <R>Patch): FieldErrors | null
```

**Wire-in:**
- `transport/http/generated/<r>.rs` create/update handlers call `validate_<r>_insertable(&input)?` BEFORE the flow.
- `frontend/src/generated/api/<r>.ts` mutations call `validate<R>Insertable(input)` BEFORE the fetch; on errors return synthetic `MeltDownResponse`-shaped error.
- `frontend/src/components/generated/forms/<r>/<Form>.vue` consumes via `computed(() => validate<R>Insertable(form.value) ?? {})` for live field error binding.

**Rule semantics:** `Required`, `MinLen(n)`, `MaxLen(n)`, `MinValue(n)`, `MaxValue(n)`, `Pattern(re)`, `OneOf([…])`, `Email`, `Url`. Defined in `crate::state::resource::ValidatorRule`. Patterns are restricted to RE2 ∩ JS RegExp intersection (no lookahead, no backreferences) so both validators interpret them identically.

`gen_level` filter: `r.gen_level >= GenLevel::Types`. Validators are useful as soon as types exist; doesn't wait for components/pages.

### Theme codegen output (Wave 10)

Driven by `ThemeConfig` in `app.ron`. Emitted by `src/codegen/theme/tokens.rs` and `src/codegen/theme/primevue.rs`. Both carry an `app.ron` blake3 hash-marker in their header — stale detection fires on `cargo check` like any other generated file.

```
frontend/src/generated/styles/tokens.css
  - CSS custom-property token file (colours, spacing, radii, etc.)
  - Source of truth for all design tokens; scoped component styles reference these vars

frontend/src/generated/plugins/primevue.ts
  - PrimeVue `definePreset()` call parameterised from ThemeConfig
  - Registered in main.ts; DO NOT import from custom/ code
```

These files are **state-driven codegen**, not static templates. They did not exist as static files prior to Wave 10.

### Icons codegen output (Wave 10)

Driven by `IconConfig` in `app.ron`. Emitted by `src/codegen/icons/emit.rs`. Carries an `app.ron` blake3 hash-marker.

```
frontend/src/generated/icons.ts
  - Typed icon-name union + icon-set export
  - Imported by components; DO NOT extend by hand
```

Also **state-driven codegen** (was a static file before Wave 10).

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

tests/common/fixtures/users.rs                (`impl Fixture for User` calling create flow)
tests/common/fixtures/mod.rs                  (barrel re-exporting every fixture module)
tests/common/mod.rs                           (shared `use canonical::*` + harness/ctx helpers)
```

Each scaffold consumes the canonical test scaffolding at `tests/common/`. The `src/testing/` feature gate was killed in 2026-04-28 — tests are top-level integration binaries and pull harness via `mod common;` declaration.

- `tests/common/harness::with_test_transaction` — always-rollback Postgres wrapper
- `tests/common/harness::run_in_test` — composes the wrapper with a `TestCtxBuilder`
- `tests/common/ctx::TestCtx<'a>` — flow-shaped test context (conn + session)
- `tests/common/fixtures::Fixture` trait + `fixture!` macro — flow-driven fixture data

In a scaffolded user-app, `canonical` is rewritten to the project's package name at scaffold time (e.g. `use myapp::*` instead of `use canonical::*`).

CLI surface: `blast gen test`, `blast gen test --flow <table>` or `<table>/<verb>`, `blast gen test --route <table>`. See `SPEC_BLAST_COMMANDS.md` and `templates/canonical/doc/SPEC_TESTING.md`.

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

## Three-Bucket Discipline

A simple two-tier model applies to the backend (frontend transition pending):

| Bucket | Who owns it | Blast behaviour |
|--------|-------------|-----------------|
| `<layer>/generated/` | Blast | Rewritten wholesale on `blast gen`; never hand-edit |
| `<layer>/<resource>/` (top-level subdirs) | User | Never read, touched, deleted, or renamed by Blast |

There is no third "vendored framework" bucket and no `vendor-update` command. The canonical template ships `auth/`, `sessions/`, etc. pre-populated at scaffold time; once scaffolded those files are user-owned forever. Framework upgrades come via `git diff` against upstream `blast/templates/canonical/` — the user merges what they want.

### Backend split

Every Rust layer with codegen looks like:

```
src/flows/
├── auth/           ← shipped at scaffold, user-owned forever
├── sessions/       ← shipped at scaffold, user-owned forever
├── <resource>/     ← user adds (checkout, notifications, etc.)
└── generated/      ← Blast-owned, regenerable
```

`mod.rs` re-exports each top-level subdir + generated:

```rust
pub mod auth;
pub mod sessions;
pub mod generated;
```

### Frontend split

Same flat two-tier model as the backend. Each top-level FE dir owns user code at the root + a `generated/` subdir owned by Blast.

```
frontend/src/
├── main.ts                   user — boots app
├── App.vue                   user — root chrome
├── components/               user
│   └── generated/            Blast (forms per resource: forms/<r>/CreateForm.vue, EditForm.vue)
├── composables/              user (auth, dialog, drawer, channel, bus, url, global-progress)
│   └── generated/            Blast (per-resource: <r>.ts with use<R>List, use<R>, use<R>Create, ...)
├── pages/                    user (Welcome, Login, Register, Dashboard, Profile, NotFound)
│   └── generated/            Blast (admin-style CRUD: <r>/{ListPage,DetailPage,CreatePage,EditPage}.vue)
├── router/
│   ├── index.ts              user — wires generated routes + user routes
│   └── generated/            Blast (routes.ts, route-names.ts, install-router-guards.ts)
├── nav/
│   └── generated/            Blast (menu.ts — sidebar entries per resource)
├── api/
│   └── generated/            Blast (per-resource fetch wrappers)
├── types/
│   └── generated/            Blast (TS interfaces from Rust DTOs)
├── validators/
│   └── generated/            Blast (TS validators mirroring Rust)
├── styles/                   user (base.css)
│   └── generated/            Blast (tokens.css, primevue-preset, icons.ts)
├── plugins/
│   └── generated/            Blast (primevue.ts boot)
└── ws/
    └── generated/            Blast (client.ts)
```

There is no `frontend/src/custom/` and no composition-hooks indirection. `main.ts`, `App.vue`, and `router/index.ts` are user-owned root files; users edit them directly to add plugins, routes, or chrome. The canonical template ships these pre-populated with sensible defaults; once scaffolded they're yours forever.

User-owned root dirs (`components/`, `composables/`, `pages/`, etc.) coexist with their `generated/` subdir. Blast NEVER touches files outside `generated/` subdirs. Resource-specific user code (e.g. a hand-written replacement `pages/users/ListPage.vue` that shadows the generated one) lives at top level of the appropriate dir.

### Blast's invariants

- `generated/` subtree is **rewritten wholesale** on `blast gen`
- Any hand-edit to a file under `generated/` gets stomped next regen
- `custom/` subtree is **never read, touched, deleted, or renamed** by Blast
- `mod.rs` at each Rust layer re-exports both; Blast regenerates only the generated side
- Vendored framework files are written once by `blast new`; user pulls upstream changes via git diff (no `vendor-update` command — killed)

## Rename Detection and Refusal

When the user renames a resource (e.g. `User` → `Account`) via the TUI wizard, Blast greps user-owned files (everything outside `src/**/generated/`) for the old symbol before writing the updated state file. If old symbols are found, Blast refuses (or loudly warns with file:line context) until the user resolves them manually. There is no magic AST patching — the layer split is intentional.

## Regeneration Behavior

`blast gen <target>`:
- `blast gen table [name]` — TUI migration wizard; emits a diesel migration (up.sql / down.sql) in `migrations/`. Does not apply; user runs `blast migrate` after.
- `blast gen migration [--custom] <name>` — empty migration scaffold (`--custom` = hand-written SQL: views/triggers/partial indexes/etc.)
- `blast gen schema` — runs `diesel print-schema`; writes `src/database/schema.rs`
- `blast gen resource [name]` — TUI wizard; writes/updates `storage/blast/state/resources/<name>.ron`. Does NOT run codegen.
- `blast gen structs` — reads schema.rs + resource state files; writes `src/structs/generated/`
- `blast gen models` — reads schema.rs + resource state files; writes `src/models/generated/` (legacy generator slated for v2 rewrite)
- `blast gen flows` — reads resource state files; writes `src/flows/generated/`
- `blast gen types [<resource>]` — reads schema.rs + resource state files; writes `frontend/src/generated/types/<r>.ts` (TS interfaces mirroring Rust DTOs)
- `blast gen api [<resource>]` — reads schema.rs + resource state files; writes `frontend/src/generated/api/<r>.ts` (typed fetch wrappers, `listX` returns `{ data, error, total, total_pages, page, page_size }`)
- `blast gen composables [<resource>]` — reads schema.rs + resource state files; writes `frontend/src/generated/composables/<r>.ts` (filter `gen_level >= GenLevel::Composables`)
- `blast gen components [<resource>]` — reads resource state files; writes `frontend/src/components/generated/forms/<r>/{CreateForm,EditForm}.vue` consuming the composable mutation factories
- `blast gen theme` — reads `app.ron` (`ThemeConfig`); writes `frontend/src/generated/styles/tokens.css` + `frontend/src/generated/plugins/primevue.ts` (hash-marker keyed off `app.ron`)
- `blast gen icons` — reads `app.ron` (`IconConfig`); writes `frontend/src/generated/icons.ts` (hash-marker keyed off `app.ron`)
- `blast gen env-example` — reads app.ron env spec; writes `.env.example`
- `blast gen governor-plugin` — reads app.ron fe_lint section; writes `frontend/scripts/governor-plugin.js` + `.rule_violations_whitelist`
- `blast gen test [--flow|--route]` — reads resource state files; scaffolds `*.test.rs` per flow + per route; idempotent on existing files
- `blast gen all` — full pipeline; step order: schema → enums → structs → models → routines → flows → http_routes → frontend_types → frontend_api → composables → theme → icons → env_example → governor_plugin. See `SPEC_BLAST_COMMANDS.md` for the exact step list. Vue components and CRUD pages are opt-in via `blast gen components` / `blast gen pages`.

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

- User's Rust source files (anything outside `src/**/generated/`)
- User's Vue/TS source files (anything outside `frontend/src/**/generated/`)
- `Cargo.toml` of user's app (except for minimal name lookup in `blast new`)

Blast reads: `storage/blast/state/*.ron` + `resources/*.ron` + `schema.rs` (Diesel output, considered stable format). Nothing else.

## What Blast OWNS (Writes)

- Everything under `src/*/generated/`
- Everything under `frontend/src/generated/` — including `styles/tokens.css`, `plugins/primevue.ts`, `icons.ts` (state-driven codegen, not static templates)
- `.env.example`
- `frontend/scripts/governor-plugin.js`
- `frontend/.rule_violations_whitelist`
- `src/database/schema.rs` (indirectly, by invoking Diesel CLI)
- `flows/generated/**/*.test.rs` (initial scaffold only; not overwritten once written)
- `transport/http/generated/**/*.test.rs` (initial scaffold only)
- `tests/fixtures/<resource>.rs` (initial scaffold only)
- All vendored framework files written by `blast new` from `templates/canonical/` (including `frontend/src/composables/bus.ts` and the `custom/` stub seeds)

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

- Generating code that references hand-written paths Blast can't see. Emit only `crate::<layer>::generated::*` references; if a user-owned module needs to be referenced, it's the user's job to wire that up.
- Overwriting parent `mod.rs` files that mix generated + user-owned. Keep parent `mod.rs` simple: `pub mod generated;` plus user-owned siblings — Blast only writes the `generated/` subtree.
- Emitting unstable output (iteration over HashMap without sorting). Always sort for determinism.
- Assuming migration has been run. Always `blast gen schema` first, error if `schema.rs` is missing.
- Writing outside `<layer>/generated/`. Ever. Blast's generators must refuse to write to user-owned paths.
- Reading deprecated paths like `target/primer/` or `target/blueprint/` — those are gone. Read `storage/blast/state/`.
- Emitting codegen without a state-hash marker in the file header.
- Timestamps, random seeds, or env-var reads inside generator logic.
- Reaching for the old string-constant emitter modules (`fe_runtime.rs`, `fe_runtime_composables.rs`, `fe_runtime_extras.rs`, `frontend_scaffold.rs`) — those ~1750 LOC of embedded string constants are gone as of Wave 10. Static FE framework files live in `blast/templates/canonical/frontend/` and are picked up by `include_dir!()`.
- Writing `frontend/src/composables/bus.ts` from a codegen pass — `bus.ts` is a static vendored file, not codegen. Per-resource composables that emit on the bus are codegen'd; `bus.ts` itself is not.
- Treating `frontend/src/generated/styles/tokens.css`, `frontend/src/generated/plugins/primevue.ts`, or `frontend/src/generated/icons.ts` as static templates — all three are state-driven codegen keyed off `app.ron` and must carry hash-markers.

## Related Specs

- `SPEC_STATE.md` — state file format, schema versioning, atomic write, upgrader contract
- `SPEC_GOVERNOR.md` — FE lint, Vite plugin wrapper emission
- `SPEC_BLAST_COMMANDS.md` — CLI surface
- `catalyst/doc/SPEC_ARCHITECTURE.md` — where generated files land
