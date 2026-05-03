# SPEC_CODEGEN

How Blast generates code post-Leptos migration. Inputs, outputs, state-hash markers, regeneration rules, two-tier ownership.

## Source-of-truth Model

Apps DO NOT depend on `catalyst` as a Cargo dep. There is no `catalyst = { path = ... }` or `catalyst = { git = ... }` line anywhere. Instead, `blast new` `git clone`s the catalyst repo (`https://github.com/ZmoleCristian/catalyst` by default, or a local path via `--dev` + `BLAST_CATALYST_DEV_PATH`) into the project root. After clone, blast applies a 3-line Cargo.toml substitution (`[package].name`, `[[bin]].name`, `output-name`) but leaves `[lib].name = "catalyst"` intact. The cloned `origin` remote is renamed to `upstream`. Every scaffolded app is its own complete framework checkout with full git history.

**Catalyst is the single source of truth.** Edit catalyst directly; commits push to its public repo. blast no longer bundles a template tree.

**Update model (end-user-time):** the user runs `git pull upstream master` from their spawned project. Conflicts only on the 3 Cargo.toml lines they substituted (`[package].name`, `[[bin]].name`, `output-name`). All other files (src/, tests/, doc/, build.rs, deps) merge cleanly because tests use the stable `[lib].name = "catalyst"` anchor and src/ uses `crate::` everywhere.

## Inputs

Blast reads from two sources:

- **`src/database/schema.rs`** — Diesel-emitted schema, regenerated from migrations. **Authoritative source for column names, types, nullability.** Blast parses this file (Diesel's `table!` macro output is stable, so a narrow parser is reliable).
- **`storage/blast/state/`** — RON state files authored by the TUI wizard or by hand. `app.ron` for app-wide policy (Blueprint); `resources/<name>.ron` per resource (Primer).

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
| `header::marker_for_resource(root, table)` | `storage/blast/state/resources/<table>.ron` | structs, models, routines, flows, http_routes, validators, leptos_pages, leptos_forms, leptos_tables, leptos data helpers |
| `header::marker_for_app(root)`              | `storage/blast/state/app.ron` | env_example, app_routes, barrels (`mod.rs` re-exports) |
| `header::marker_for_schema(root)`           | `src/database/schema.rs` | reserved (schema-driven passes that don't have a stable Primer source) |

The marker is **Rust-only** — comments use `// AUTO-GENERATED ...`. The HTML/Vue marker variant (`<!-- AUTO-GENERATED ... -->`) is gone. Leptos UI is Rust source compiled to WASM; there are no `.vue` / `.ts` / `.css` files to mark.

The marker is parsed back at compile time by the user app's `build.rs` (template at `crate::codegen::build_rs_template`, source at `crate::codegen::build_rs_template_src.rs.tmpl`). On hash mismatch `build.rs` calls `panic!` so `cargo check` / `cargo build` / `cargo test` all hard-fail with an actionable message.

The parser enforces `hash.len() == 64 && all-hex` — exactly BLAKE3 hex digest length. Truncated, oversized, or non-hex hashes don't false-positive as valid markers; they're treated as no marker found (which trips a different lint path). Same enforcement on the test-side `parse_marker` in `header.rs::tests` and the inline test helper in `build_rs_template.rs::tests` for symmetric semantics.

Users cannot forget to regen. Stale codegen is a compile error.

## Generation Level (per-resource cut-off)

Each resource declares a `gen_level` in its RON state file controlling how far codegen propagates. Levels are linear and monotonic — picking level N implies all levels < N. Each pass filters `r.gen_level >= GenLevel::X` at runner entry.

Authoritative enum: `crate::state::gen_level::GenLevel`. Default `Composables`.

```
Struct < Model < Route < Types < Composables < Components < Pages
```

The level names are kept from the pre-leptos design for state-file backward compatibility (no schema_version bump). Their cut-off semantics map onto the new pipeline:

| Level | Cut-off behaviour |
|-------|-------------------|
| `Struct` | structs only. Data shape, no persistence, no transport, no UI. |
| `Model` | + Diesel CRUD (`models/generated/`). |
| `Route` | + routines + flows + REST handlers (full BE CRUD). |
| `Types` | + validators (the single Rust validator that compiles to server + WASM). |
| `Composables` (default) | + leptos data helpers (`transport/leptos/data/generated/<r>.rs`). |
| `Components` | + leptos forms (`views/components/generated/forms/<r>/`). |
| `Pages` | + leptos pages + tables + app_routes barrel entry (full admin-style CRUD UI). |

Page generation philosophy: generated pages target **admin-grade / internal-tool quality**, not production user-facing UI. Apps with custom branding shadow generated pages by writing their own `pages/<r>.rs` at top level and routing to it instead.

**Level-downgrade behavior:** if the user lowers `gen_level` (e.g. `Pages` → `Composables`), the next `blast gen` run STOPS emitting at the new level but does NOT delete files emitted at the previous level. `crate::commands::gen_all::warn_on_orphan_generated` walks each resource's expected output paths and warns about stale files above the current level. The user must delete them manually.

## Pipeline order

`blast gen all` runs the steps in `crate::commands::gen_all::run` in this order:

```
schema → enums → structs → models → routines → flows → http_routes → validators
       → leptos_pages → leptos_forms → leptos_tables → app_routes → env_example
```

Hard ordering rules:

- `schema` must run first — every other pass parses `src/database/schema.rs` for column types.
- `enums` runs before `structs` — Postgres `CREATE TYPE` enum scanning emits `src/structs/generated/enums/` so per-resource structs can reference the type names.
- `structs` runs before `models`, `validators`, and any consumer that imports `<R>Insertable`/`<R>Patch`/`<R>Public`.
- `routines` runs before `flows` (flows wrap routines under a `Crank` policy).
- `http_routes` and the leptos passes both run after `validators` — they import the validator function for create/update.
- `app_routes` runs after `leptos_pages` — it concatenates the per-resource `<Route path=...>` entries into a barrel.
- `env_example` runs last — independent of every other pass.

## Outputs (Per Resource)

For a resource `users` with verbs `[List, Get, Create, Update, Delete]`, gen_level `Pages`:

### Rust (server + WASM)

Naming convention for projection structs:

```
<TypeStem><Variant>
```

| Variant | Type name | Role |
|---------|-----------|------|
| `Db` | `User` | Diesel `Queryable` row |
| `Insertable` | `UserInsertable` | Diesel `Insertable` for `create` |
| `Patch` | `UserPatch` | Diesel `AsChangeset`, all fields `Option<T>` |
| `Public` | `UserPublic` | Response shape returned to authenticated users |
| `Admin` | `UserAdmin` | Response shape returned through admin-only routes |
| (filter) | `UserFilter` | List endpoint query shape |

Output paths:

```
src/structs/generated/users.rs           User, UserInsertable, UserPatch, UserPublic, UserAdmin, UserFilter
src/structs/generated/mod.rs             barrel — alphabetical re-exports + `pub mod validators;` if any emitted
src/structs/generated/enums/<x>.rs       per Postgres CREATE TYPE; Rust enum + Diesel FromSql/ToSql
src/structs/generated/enums/mod.rs       enum barrel
src/structs/generated/validators/<r>.rs  validate_<r>_insertable, validate_<r>_patch (single Rust source, runs in server + WASM)
src/structs/generated/validators/mod.rs  validators barrel

src/models/generated/<r>.rs              Diesel CRUD: list/get/create/update/delete + auto-conn impl + fluent UserQuery
src/models/generated/mod.rs

src/routines/generated/<r>/<verb>.rs     atomic capability — one file per verb. owns ctx.conn(), calls model, maps Row→Public
src/routines/generated/<r>/mod.rs        per-resource verb barrel
src/routines/generated/mod.rs            top-level barrel

src/flows/generated/<r>/<verb>.rs        thin wrapper: auth check + Crank::none().run(|| routines::generated::<r>::<verb>::run(ctx, args.clone()))
src/flows/generated/<r>/mod.rs
src/flows/generated/mod.rs

src/transport/http/generated/<r>.rs      REST handlers for /api/<r>: list, get_one, create, update, delete_one
                                         each calls validate_<r>_insertable(&input)? before flow::create::run(...) (and matching for update)
src/transport/http/generated/router.rs   nests every resource: Router::new().nest("/<r>", super::<r>::router())
src/transport/http/generated/mod.rs
```

`http_routes` emits to `src/transport/http/generated/<r>.rs` (flat per-resource). Mounted by canonical's `main.rs` under `/api/*`.

### Leptos (UI side, planned phase 4 → real emitters)

Output paths under `src/transport/leptos/`. The runners exist and are wired into the pipeline; real bodies land per resource verb in subsequent iterations (current state: stubs returning empty `EmitReport`).

```
src/transport/leptos/data/generated/<r>.rs                   isomorphic helpers: load_<r>_list, load_<r>, do_<r>_create, do_<r>_update, do_<r>_delete
                                                              cfg-branched: SSR calls flow direct via expect_context::<Ctx>(); WASM calls /api/<r> via api_client
src/transport/leptos/data/generated/mod.rs

src/transport/leptos/pages/generated/<r>/list.rs             list page (consumes leptos-struct-table on UserPublic)
src/transport/leptos/pages/generated/<r>/detail.rs           detail page
src/transport/leptos/pages/generated/<r>/create.rs           create page (renders <UserCreateForm>)
src/transport/leptos/pages/generated/<r>/edit.rs             edit page (renders <UserEditForm>)
src/transport/leptos/pages/generated/<r>/mod.rs

src/views/components/generated/forms/<r>/create_form.rs   leptos-form derive on UserInsertable
src/views/components/generated/forms/<r>/edit_form.rs     leptos-form derive on UserPatch
src/views/components/generated/forms/<r>/mod.rs

src/views/components/generated/tables/<r>.rs              leptos-struct-table derive on UserPublic
src/views/components/generated/tables/mod.rs

src/transport/leptos/routes/generated.rs     barrel of leptos_router <Route path=path!("/<r>") view=<R>ListPage/> entries; one entry per gen_level≥Pages resource
```

Pages and forms gate on per-verb Primer flags `emit_html_page` / `emit_rest_api`. A resource with `emit_rest_api: false` for `Create` skips the `POST /api/<r>` REST handler but still emits the page form (and vice versa).

### Postgres ENUM end-to-end

When user adds `CREATE TYPE my_status AS ENUM ('a', 'b', 'c');` to a migration, the `enums` codegen pass (`src/codegen/enums/`) detects it via `scan::scan_project_enums(project_root)` and emits a Role-shape Rust file:

```
src/structs/generated/enums/my_status.rs
  - pub enum MyStatus { A, B, C }
  - impl MyStatus { fn as_str() -> &'static str; fn parse(&str) -> Result<Self, MeltDown>; }
  - impl FromSql<MyStatus, Pg> for MyStatus
  - impl ToSql<MyStatus, Pg> for MyStatus
  - sql_type tied to crate::database::schema::sql_types::MyStatus
```

The Diesel `sql_types` marker struct lives in canonical's `src/database/schema.rs` (emitted by `diesel print-schema`) — `STRUCTS:22` build.rs lint exempts that file specifically.

**Skip-emission for hand-written enums:** if a Rust enum with the matching PascalCase name already exists under `src/structs/**` (excluding any `generated/` subtree), the codegen pass skips emission for that SQL `CREATE TYPE`. Detection is `existing_user_enums(project_root) -> HashSet<String>`. Canonical's hand-rolled `Role` enum at `src/structs/auth/role.rs` is treated this way for `CREATE TYPE user_role`.

Tests: 6 inline tests at `blast/src/codegen/enums/runner.rs:160-319` cover the full pipeline against fixture migrations.

### Validators (single source)

Driven by `FieldState.validators: BTreeSet<ValidatorRule>` in each Primer file. Codegen pass at `src/codegen/validators/` emits a single Rust validator per resource:

```
src/structs/generated/validators/<r>.rs
  - pub fn validate_<r>_insertable(input: &<R>Insertable) -> Result<(), MeltDown>
  - pub fn validate_<r>_patch(input: &<R>Patch) -> Result<(), MeltDown>
  - once_cell::sync::Lazy<Regex> constants for any Pattern/Email/Url rules
```

The whole crate compiles to WASM (`cargo build --target wasm32-unknown-unknown --lib`), so the same function runs on the server (called from REST handlers) and in the browser (called from leptos forms before `Action::dispatch`). One source. No drift. **No TS validator file is emitted.**

`gen_level` filter: `r.gen_level >= GenLevel::Types`. Validators are useful as soon as types exist.

Wire-in points:

- `src/transport/http/generated/<r>.rs` — create handler calls `validate_<r>_insertable(&input)?` BEFORE `flow::create::run(...)`. Update handler calls `validate_<r>_patch(&patch)?` BEFORE `flow::update::run(...)`. Order tested in `http_routes::tests::create_handler_calls_validator_before_flow`.
- `src/views/components/generated/forms/<r>/create_form.rs` — `on:submit` handler parses signal values into `<R>Insertable` (synchronously, in an IIFE returning `Result<<R>Insertable, MeltDown>`), then `validate_<r>_insertable(&parsed)` (synchronously, returns `Result<(), MeltDown>`), THEN `spawn_local(async move { do_<r>_create(parsed).await; ... })`. NOT `Action::new` — that pattern deadlocked the wasm event loop. Cuts a server roundtrip on locally-detectable bad input.

Full spec: `catalyst/doc/SPEC_VALIDATORS.md`.

### Misc output

```
.env.example                               from app.ron env_spec section, Blueprint hash marker
```

The user-app `build.rs` is **not** regenerated by `blast gen all` — it's emitted once by `blast new` (template at `src/codegen/build_rs_template_src.rs.tmpl`, runner at `src/codegen/build_rs_template.rs`) and committed. It is intentionally short, has no external deps beyond `blake3`, and walks `WATCHED_DIRS` looking for `// AUTO-GENERATED from ...` markers.

`WATCHED_DIRS` covers `src/{structs,models,flows,transport/http,transport/ws}/generated`, the transport/leptos generated subdirs (`pages`, `data`, `routes`), and the views layer (`src/views/components/generated`).

## What Blast Does NOT Emit Anymore

The pre-leptos pipeline emitted to a `frontend/` directory that no longer exists. The following passes were removed in phase 1:

- **Vue SFC files** (`.vue`) — components, pages, forms, list views.
- **TypeScript types** (`frontend/src/types/generated/<r>.ts` — interfaces mirroring Rust DTOs). The Rust struct compiled to WASM is the type now.
- **TS API clients** (`frontend/src/api/generated/<r>.ts`). Replaced by isomorphic data helpers in `src/transport/leptos/data/generated/<r>.rs`.
- **TS composables** (`frontend/src/composables/generated/<r>.ts`). Replaced by `RwSignal<Option<Result<T, MeltDown>>>` + `#[cfg(target_arch = "wasm32")] Effect::new(spawn_local(load_*))` consumers of the data helpers in pages, and `spawn_local` directly in form `on:submit` handlers. NOT `Resource::new`/`LocalResource::new`/`Action::new` — see SPEC_LEPTOS for the full rationale (Resource needs Serialize MeltDown, LocalResource panics on SSR via js-sys, Action+Effect deadlocks wasm event loop on submit).
- **TS validators** (`frontend/src/validators/generated/<r>.ts`). Single Rust validator runs in WASM.
- **vue-router config** (`frontend/src/router/generated/`). Replaced by `app_routes` codegen emitting `leptos_router` `<Route>` entries.
- **Governor plugin shim** (`frontend/scripts/governor-plugin.js`) and `.rule_violations_whitelist`. Governor is gone — replaced by a planned `LEPTOS:N` rule family in canonical's `build.rs`.
- **HTML-marker variant of `header.rs`**. Marker is now Rust-only (`// AUTO-GENERATED ...`).

The entire `frontend/` directory is gone from `catalyst/`. No node, no npm, no Vite, no PrimeVue, no Tailwind.

## Two-tier Ownership

Backend and Leptos UI follow the same two-tier model.

| Bucket | Who owns it | Blast behaviour |
|--------|-------------|-----------------|
| `<layer>/generated/` | Blast | Rewritten wholesale on `blast gen`; never hand-edit |
| `<layer>/<resource>/` (top-level subdirs) | User | Never read, touched, deleted, or renamed by Blast |

There is no `custom/` subdir, no third "vendored framework" bucket, and no `vendor-update` command. The canonical template ships hand-written `<layer>/auth/`, `<layer>/sessions/`, etc. pre-populated at scaffold time; once scaffolded those files are user-owned forever. Framework upgrades come via `git diff` against upstream `catablast/catalyst/`.

### Backend layout

Every Rust layer with codegen looks like:

```
src/flows/
├── auth/           ← shipped at scaffold, user-owned forever
├── sessions/       ← shipped at scaffold, user-owned forever
├── <resource>/     ← user adds (checkout, notifications, etc.)
└── generated/      ← Blast-owned, regenerable
```

Same for `models/`, `routines/`, `structs/`, `transport/http/`, `transport/ws/`.

`mod.rs` re-exports each top-level subdir + generated:

```rust
pub mod auth;
pub mod sessions;
pub mod generated;
```

### Leptos layout

The leptos surface spans **two layers**:

- `src/transport/leptos/` — entry points + flow bridge (pages, app, data helpers, api_client).
- `src/views/` — UI primitives (components, builders, signals).

```
src/transport/leptos/
├── vendored/                     user-owned
│   ├── mod.rs / app.rs / client.rs / api_client.rs / auth_storage.rs
│   ├── pages/                    user-owned baseline (welcome, login, register, dashboard, profile)
│   ├── data/                     user-owned isomorphic helpers (auth.rs)
│   └── routes/                   user-owned RouteName entries
├── generated/                    Blast-owned
│   ├── pages/<r>/                admin-style CRUD pages (list/detail/create/edit)
│   ├── data/<r>.rs               per-resource isomorphic helpers (load_<r>_list, do_<r>_create, ...)
│   └── routes/                   <Route> entry concatenation
└── custom/                       user-owned overrides

src/views/
├── components/
│   ├── vendored/                 user-owned UI components
│   │   ├── auth_guard.rs / error_banner.rs / page_shell.rs / app_shell.rs / button.rs / ...
│   │   └── cells/                value renderers (BadgeCell, DateCell, MoneyCell, ...)
│   └── generated/                Blast-owned
│       ├── forms/<r>/            create_form.rs, edit_form.rs (native HTML + spawn_local)
│       └── nav/                  app_nav.rs (role-gated link entries)
├── builders/                     TableBuilder / FormBuilder / ListBuilder / SelectBuilder / DetailBuilder / StatBuilder
│   └── vendored/                 user-owned
└── signals/                      reactivity primitives + signal stores
    └── vendored/                 user-owned (use_session, use_resource_effect, use_polled_resource, use_live_resource, use_url_list_state, use_toast, ...)
```

`TRANSPORT:23` (no `State<Ctx>`) is scoped to `src/transport/http/` only — leptos pages use `expect_context::<Ctx>()` (Leptos context system) and don't trip the lint.

### Blast's invariants

- `generated/` subtree is **rewritten wholesale** on `blast gen`
- Any hand-edit to a file under `generated/` gets stomped next regen
- Top-level user-owned subdirs are **never read, touched, deleted, or renamed** by Blast
- `mod.rs` at each Rust layer re-exports both; Blast regenerates only the generated side
- Vendored framework files are written once by `blast new`; user pulls upstream changes via git diff against upstream `catablast/catalyst/`

## Determinism

Generated output is **byte-identical for byte-identical state input** across runs, machines, and Blast versions. Same state files + same Blast version → identical files. This matters for clean diffs on regen, reviewability, git hygiene, and state-hash marker integrity.

Rules enforced by all generators:

- Use `BTreeMap` everywhere iteration order matters; never `HashMap`.
- Sort before emit: struct fields in canonical order (PK first, then alphabetical), `mod.rs` re-exports alphabetical, layer entries alphabetical.
- No clocks in codegen output (no `generated_at` timestamps in generated Rust files).
- No env vars in codegen logic.
- No random values.

## Generation Strategy

Current: `format!()` string templates. Each generator (`structs/emitter/`, `models/builder.rs`, `flows.rs`, etc.) builds output via nested `format!` calls.

Pros:
- Simple. No AST library. Readable.
- Fast.

Cons:
- Verbose for complex generators.
- No type-safety on output correctness (you can emit broken Rust).
- Escape-your-own-braces pain with `format!` syntax.

Migration path (if it gets painful): `quote!` + `syn` for AST-based generation. Not a blocker.

## What Blast Does NOT Parse

- User's hand-written Rust source files (anything outside `src/**/generated/`)
- User's hand-written `.scss` files (`style/main.scss`, `style/tokens.scss`, per-component `.module.scss`)
- `Cargo.toml` of user's app (except for minimal name lookup in `blast new`)

Blast reads: `storage/blast/state/*.ron` + `resources/*.ron` + `schema.rs`. Nothing else.

## What Blast OWNS (Writes)

- Everything under `src/{structs,models,routines,flows,transport/http,transport/ws}/generated/`
- Everything under `src/structs/generated/{enums,validators}/`
- Everything under `src/transport/leptos/{pages,data,routes}/generated/`
- Everything under `src/views/components/generated/{forms,nav}/`
- `.env.example`
- `src/database/schema.rs` (indirectly, by invoking Diesel CLI on `blast gen schema`)
- All vendored framework files written by `blast new` from `catalyst/` (one-shot at scaffold time)

## What Blast DOES NOT Write

- Anything outside `<layer>/generated/` subdirs
- User's hand-written Leptos components / pages / scss
- `Cargo.toml` (after `blast new` initial scaffold)
- Migrations (user writes them; Blast only runs them)
- `.env` (secrets + runtime values)
- `style/main.scss`, `style/tokens.scss`, `style/base.scss`, per-component `.module.scss` (CSS is user-owned end-to-end — no theme codegen pass)
- `storage/blast/state/` files except via TUI wizard — never silently rewritten during codegen

## Rename Detection and Refusal

When the user renames a resource (e.g. `User` → `Account`) via the TUI wizard, Blast greps user-owned files (everything outside `src/**/generated/`) for the old symbol before writing the updated state file. If old symbols are found, Blast refuses (or loudly warns with file:line context) until the user resolves them manually. There is no magic AST patching — the layer split is intentional.

## Anti-Patterns (for Blast maintainers)

- Emitting `.vue`, `.ts`, `.css`, `.js`, or any non-`.rs` file from a codegen pass. The frontend stack is gone — Leptos is Rust source.
- Emitting a TS validator pair next to the Rust validator. Single Rust source compiled to WASM is the contract.
- Emitting an HTML-comment marker variant. The HTML-marker codepath was deleted in phase 1; never resurrect.
- Generating code that references hand-written paths Blast can't see. Emit only `crate::<layer>::generated::*` references; if a user-owned module needs to be referenced, it's the user's job to wire that up.
- Overwriting parent `mod.rs` files that mix generated + user-owned. Keep parent `mod.rs` simple: `pub mod generated;` plus user-owned siblings — Blast only writes the `generated/` subtree.
- Emitting unstable output (iteration over `HashMap` without sorting). Always sort for determinism.
- Assuming migration has been run. Always `blast gen schema` first, error if `schema.rs` is missing.
- Writing outside `<layer>/generated/`. Ever.
- Reading deprecated paths like `target/primer/` or `target/blueprint/` — those are gone. Read `storage/blast/state/`.
- Emitting codegen without a state-hash marker in the file header.
- Timestamps, random seeds, or env-var reads inside generator logic.
- Writing into `frontend/` — the dir doesn't exist. The whole Vue/TS/Vite stack was deleted.
- Touching `style/*.scss` from any codegen pass — CSS is user-owned end-to-end. No `tokens.css` codegen, no PrimeVue preset codegen.
- Reaching for `governor_plugin` codegen — Governor is gone. Lint moves into canonical's `build.rs` `LEPTOS:N` rule family.

## Related Specs

- `SPEC_STATE.md` — state file format, schema versioning, atomic write, upgrader contract
- `SPEC_BLAST_COMMANDS.md` — CLI surface, `blast gen all` step list
- `catalyst/doc/SPEC_LEPTOS.md` — Leptos UI integration, where pages and forms live
- `catalyst/doc/SPEC_VALIDATORS.md` — single-source validator codegen
- `catalyst/doc/SPEC_CSS.md` — scss + stylance pipeline (user-owned, not codegen)
- `catalyst/doc/SPEC_PRIMER.md` — per-resource RON shape
- `catalyst/doc/SPEC_ARCHITECTURE.md` — where generated files land in the layer graph
