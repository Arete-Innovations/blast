# SPEC_BLAST_COMMANDS

Full command surface of the `blast` CLI. TUI flows, dashboard, and interactive menu. Designed so that CLI and dashboard are two surfaces over the same underlying behavior.

## Top-Level Commands

```
blast new <name>                 # scaffold via git clone catalyst + 3-line Cargo.toml sub
blast init [<name>]              # in-place scaffolder (cwd or <name>/)

blast migration [name]           # create a new Diesel migration skeleton
blast migrate                    # run pending migrations
blast rollback [--steps N]       # roll back N migrations (default 1)
blast seed [file]                # run seed SQL file
blast schema                     # regenerate src/database/schema.rs from DB

blast gen                        # prints clap help for `blast gen` (no picker)
blast gen <target>               # see targets below
blast gen all                    # full pipeline

blast check                      # run Governor lint on frontend

blast run                        # dev server (backend + Vite HMR proxy)
blast run-prod                   # production server (backend only, serving dist/)
blast stop                       # tear down BE + FE daemons (pkill --pgroup) + cleanup pid files
blast watch                      # BE: cargo-watch -x run --watch src --watch Cargo.toml
                                 # FE: vite dev (auto npm-install on first start)
                                 # Both spawn as detached pgroup leaders — pid files at storage/blast/{server,frontend}.pid

blast dashboard                  # Zellij-based TUI dashboard
blast cli                        # ratatui list-select menu (Command registry)

blast fuses                      # interactive fuses TUI
blast fuses list                 # list registered fuses
blast fuses toggle <name>        # flip enabled flag
blast fuses run <name>           # trigger immediate run (bypass schedule)
blast fuses logs <name>          # show recent run logs

blast log [truncate|view LEVEL]  # tail/manage blast logs

blast build                      # lint frontend + vite build + cargo build --release
blast package                    # archive release binary + frontend/dist + systemd unit
blast test [filter]              # create test DB, migrate, run suite, drop test DB
blast test --no-drop             # keep test DB after run (for inspection)

blast refresh                    # reinstall deps, rerun init pipeline
blast toggle-env                 # flip Env::Dev <-> Env::Prod
blast help                       # top-level help
```

## `blast gen` Targets

```
blast gen schema                  # diesel migration run + print-schema
blast gen structs                 # schema.rs + resource state → src/structs/generated/
blast gen models                  # schema.rs + resource state → src/models/generated/
blast gen routines                # resource state → src/routines/generated/<r>/<verb>.rs (each wraps the model fn, called by flows)
blast gen flows                   # resource state → src/flows/generated/ (each verb wraps the routine in Crank::none() + auth check)
blast gen types [<resource>]     # resource state → frontend/src/generated/types/<r>.ts
blast gen api [<resource>]       # resource state → frontend/src/generated/api/<r>.ts (typed fetch wrappers)
blast gen composables [<resource>]  # resource state → frontend/src/generated/composables/<r>.ts (use<R>List, use<R>, useCreate<R>, useUpdate<R>, useDelete<R>)
blast gen validators [<resource>]  # resource state validators[] → src/structs/generated/validators/<r>.rs + frontend/src/generated/validators/<r>.ts (paired Rust+TS field validators)
blast gen components [<resource>] # resource state → frontend/src/components/generated/forms/<r>/{CreateForm,EditForm}.vue
blast gen pages [<resource>]     # resource state → frontend/src/pages/generated/<r>/{ListPage,DetailPage,CreatePage,EditPage}.vue
blast gen env-example             # app state env spec → .env.example
blast gen governor-plugin         # app state fe_lint section → frontend/scripts/governor-plugin.js + .rule_violations_whitelist
blast gen fe-scaffold             # seed tokens.css, base.css, primevue.ts (idempotent — first-run seed)
blast gen test [--flow|--route]   # resource state → *.test.rs scaffolds per flow + per route (idempotent on existing files)
blast gen all                     # full pipeline (see below)
```

All `blast gen` targets read from `storage/blast/state/` (see `SPEC_STATE.md`). There is no `blast gen primer` or `blast gen blueprint` — the DSL sub-crates are gone.

### `blast gen all` pipeline

`blast gen all` runs the ordered steps below. Each step calls a dedicated codegen module and reports `{written, skipped}` counts back to the sink. On any step's failure the pipeline aborts; no retries (that's `blast init`'s job).

```
1.  schema generation           (diesel print-schema → src/database/schema.rs; preserves `pub use` lines in src/database/mod.rs)
2.  enums generation            (codegen::enums::run — scans CREATE TYPE in migrations)
3.  structs generation          (codegen::structs::run)
4.  models generation           (codegen::models::run — emits the per-resource model layer + auto-conn impls + fluent query builder)
5.  routines generation         (codegen::routines::run — per-verb stubs that wrap models, called by flows)
6.  flows generation            (codegen::flows::run — auth check + Crank::none wrapping the routine)
7.  http routes generation      (codegen::http_routes::run)
8.  frontend types generation   (codegen::frontend_types::run)
9.  frontend api generation     (codegen::frontend_api::run — one-line fetchers per resource that delegate to apiFetch)
10. composables generation      (codegen::composables::run — Vue 3 reactive composables per resource: list/get/create/update/delete)
11. validators generation       (codegen::validators::run — paired Rust + TS field validators from FieldState.validators)
12. components generation       (codegen::components::run — Vue Create/Edit form components per resource at gen_level >= Components)
13. pages generation             (codegen::pages::run — Vue List/Detail/Create/Edit pages per resource at gen_level >= Pages)
14. frontend router generation  (codegen::frontend_router::run — routes.ts table that maps resource pages to vue-router entries; gen_level >= Pages)
15. .env.example generation     (codegen::env_example::run)
16. governor plugin emission    (codegen::governor_plugin::run)
```

Steps short-circuit cleanly when zero resource state files are declared (logged as "no resources declared; skipping"). Each pass filters resources by `gen_level`:
- routines + flows: `>= Route`
- frontend types/api/validators: `>= Types`
- composables: `>= Composables` (default for new resources)
- components: `>= Components`
- pages + frontend_router: `>= Pages`

The `migration` wizard offers a `gen_level` picker on its Form screen — select `Pages` to scaffold a fully-wired CRUD UI end-to-end without a separate `blast gen pages` invocation. Theme tokens and icons are NOT codegen — they ship as user-owned files (`frontend/src/{styles/tokens.css, plugins/primevue.ts, icons.ts}`).

Implementation lives in `src/commands/gen_all.rs` as `pub fn run(args, config, sink, progress) -> BlastResult<Outcome>`. `Outcome` carries cumulative `steps_run`, `files_written`, `files_skipped`.

## TUI Flows

### `blast migration` — the chained new-table wizard

One ratatui state-machine wizard (no dialoguer) that handles every step from migration SQL to working CRUD endpoints. Lives under `src/wizards/new_table/`. Three screens, linear progression, all Tab-navigable.

**Screen 1 — Form:**
- Table name (snake_case input, validated)
- Auto-features (4 individually-focusable checkboxes): `id BIGSERIAL PRIMARY KEY`, `created_at`, `updated_at`, `deleted_at` (soft-delete)
- Codegen depth picker (`gen_level`): cycles Struct / Model / Route / Types / Composables / Components / Pages
- Verb checkboxes (5 individually-focusable): List / Get / Create / Update / Delete
- `[ Next: Columns → ]` button

**Screen 2 — Columns loop:** existing column list at top, draft form below. Per-column draft:
- Name input (snake_case)
- Type picker — cycles TEXT / VARCHAR(255) / INTEGER / BIGINT / BOOLEAN / TIMESTAMPTZ / UUID / JSONB / NUMERIC + dynamic Enum entries (from existing `CREATE TYPE`) + dynamic FK entries (`BIGINT REFERENCES <table>(id)` for every existing table). Enum entries only appear when the project actually declares enums.
- `NOT NULL` checkbox (default on)
- `Public-visible` checkbox (default OFF — opt-in to expose; hard-forced off at add-time when the name matches `password_hash`, `*_secret`, `*_token`, `*_key` regardless of user toggle, so sensitive cols can never reach the Public variant by accident)
- Validator picker — cycles None / Required / Email / MaxLen(255)
- `[ + Add column ]`, `[ – Delete last column ]`, `[ ← Back ]`, `[ Done — Preview → ]`

**Screen 3 — Preview + commit:** shows the generated `up.sql`, `down.sql`, and `storage/blast/state/resources/<name>.ron` side-by-side. Two buttons: `[ ← Back ]` or `[ Commit + Run Pipeline → ]`.

**On commit, the wizard chains the full pipeline:**

1. Writes `migrations/<timestamp>_create_<table>/up.sql` + `down.sql`.
2. Writes `storage/blast/state/resources/<table>.ron` via atomic `.tmp` + rename. Per-column policy is mapped from the wizard:
   - `Public-visible = true` → variants `Db, Insertable, Patch, Public, Admin`
   - `Public-visible = false` → variants `Db, Insertable, Patch, Admin` (no Public)
   - Auto-features get system-correct variants (id is `Db, Public, Admin`; `created_at`/`updated_at` same; `deleted_at` is `Db, Admin`).
   - Validator picker maps to `ValidatorRule::{Required, Email, MaxLen(255)}`.
   - All enabled verbs default to `AuthMode::AuthRequired`.
3. `blast migrate` (applies the migration).
4. `blast gen schema` (refreshes `src/database/schema.rs`).
5. `blast gen all` (runs the full codegen pipeline).

**Keys:** Tab/Shift-Tab cycle focus, Space toggles checkboxes, ←/→ cycle pickers, Enter activates buttons, Esc cancels.

**What the wizard does NOT cover (hand-edit RON for these):**
- Per-verb auth modes other than `AuthRequired` (admin_only / scoped_to / roles).
- Variant fine-grain (e.g. Insertable but not Patch).
- WebSocket events.
- Validator rules beyond Required/Email/MaxLen(255) (Pattern, OneOf, MinLen, MinValue, MaxValue).
- schema_diff (drift between `schema.rs` and RON). `blast gen all` already errors loud on hash mismatch.

These were intentionally cut to keep the wizard ship-fast. Hand-editing the RON file (which is short and human-readable) is the escape hatch.

### `blast gen` (no args)

Launches a ratatui list-select menu listing every `GenCmd` variant. User picks → the matched variant runs through the same `commands::execute` dispatch as the CLI. The new-table chained wizard is reachable via `blast migration`, not via this picker.

## Dashboard (`blast dashboard`)

Zellij layout (`storage/blast/dashboard.kdl`):

- **Top pane:** status line (env, last command result, log file tail)
- **Main pane:** interactive menu (same as `blast cli` embedded)
- **Side pane:** live log viewer (ratatui-based, `storage/blast/blast.log`)
- **Bottom pane:** fuses live table (`blast fuses` auto-refresh view)

Process: runs `zellij --layout ...kdl`, replaces current process. The panes are normal `blast` subprocesses. There is no special "dashboard mode" inside command bodies. The old `logger.rs` "dashboard suppress stdout" branch is dead — commands output through the injected `Sink`, not via a global logger with a dashboard gate.

Dashboard is a presentation layer. All actual work goes through the same `run()` path as CLI.

## Interactive CLI (`blast cli`)

Ratatui list-select menu backed by a hand-picked subset of the `Command` registry. Only operations a user actually drives interactively from inside the dashboard live here. CI-scope commands (`blast build`, `blast package`), recursive ones (`blast dashboard` — the menu IS a dashboard pane), codegen sub-passes subsumed by `gen all` (`schema`, `structs`, `models`, `governor-plugin`), and `blast new`/`init` (project must not already exist) stay reachable as `blast <subcommand>` from the shell, never as menu entries.

19 entries total:

```
[APP] Run Server (dev)              [DB]   New Migration       [LOG]     View logs
[APP] Run Server (prod)             [DB]   Migrate             [LOG]     Truncate Logs
[APP] Watch (BE+FE HMR)             [DB]   Rollback            [LINT]    Governor Check
[APP] Stop Server                   [DB]   Seed                [LINT]    Governor Check (verbose)
[APP] Refresh                       [FUSES] Manage fuses (TUI) [ARSENAL] Scan & Write JSON
[APP] Toggle Dev/Prod               [FUSES] List fuses         [ARSENAL] Serve MCP (stdio)
[CODEGEN] Gen All (full pipeline)   [FUSES] Toggle fuse        [Exit]    Kill Session
                                    [FUSES] Run fuse now
                                    [FUSES] Fuse logs
                                    [FUSES] Live fuses table
```

Up/Down/j/k navigation, Enter to confirm, Esc/Ctrl-C to cancel. `text_input::ask` widget covers fuse-name + log-level prompts. Selection drops into that command's handler — possibly another TUI, possibly direct execution.

Adding a new entry: add to `MENU_ITEMS` in `src/interactive.rs` AND add a `resolve_selection` arm AND list it in the parity test (`non_interactive_menu_items_all_resolve` or the interactive sub-prompt list). Compile-time test enforces no orphan labels.

## Post-Scaffold Initialization Pipeline

Run after `blast new` or `blast init` to bring a freshly-scaffolded (or freshly-cloned) project to a working state. Also re-run via `blast refresh`.

```
Step 1/7: Dependency check               (cargo, diesel_cli, node, zellij available?)
Step 2/7: Database: rollback + migrate   (idempotent reset)
Step 3/7: Seed data (if seed file)
Step 4/7: Schema generation              (blast gen schema)
Step 5/7: Full codegen pipeline          (blast gen all)
Step 6/7: Governor plugin + env-example  (included in gen all)
Step 7/7: Arsenal scan                   (blast arsenal)
```

Retries critical steps (schema, codegen) up to 3 times on failure before exiting.

Progress shown via `indicatif` progress bars. File log at `storage/blast/init.log`.

## `blast new <name>`

Scaffolds a fresh Catablast app by `git clone`ing the catalyst framework (default: `https://github.com/ZmoleCristian/catalyst` master, override with `BLAST_CATALYST_DEV_PATH` + `--dev` for local-path clone). The cloned `origin` is renamed to `upstream` so the user can add their own `origin` remote later. blast does NOT bake any template tree.

```
blast new <name> [--dev] [--db-url <url>] [--no-test-db] [--force]
```

| Flag | Meaning |
|------|---------|
| `--dev` | Clone from local catalyst path in `BLAST_CATALYST_DEV_PATH` env var instead of the public git URL. Errors if the env var is unset. |
| `--db-url <url>` | Postgres URL for the new project. If omitted, prompts interactively. |
| `--no-test-db` | Skip creation of the `<dbname>_test` database and `.env.test` file. |
| `--force` | Drop and recreate target databases if they already contain tables. |
| `--no-warmup` | Skip `cargo build` warmup. Still execs into the dashboard at the new project root afterwards (use `BLAST_NO_TUI_FOR_TESTS=1` to also suppress the dashboard exec — internal-only escape hatch for verification scripts). |

After clone, scaffold applies a 3-line Cargo.toml substitution (`[package].name`, `[[bin]].name`, `[package.metadata.leptos] output-name`) replacing `catalyst` with `<project_name>`. **`[lib].name = "catalyst"` STAYS** — anchors `tests/*.rs use catalyst::*` so source/tests are byte-identical with upstream catalyst across all forks. `git pull upstream master` from a spawned project only conflicts on those 3 Cargo.toml lines.

Full implementation in `src/project/{mod, scaffold, templates, preflight, post_install, db_bootstrap}.rs`.

NOT done by `blast new`:
- Run initial migrations (`blast init` does that as a second step)
- Install node deps (user runs `npm install` in `frontend/`)

## `blast init [<name>]`

In-place scaffolder. Mirrors `blast new` exactly but defaults to the current working directory when `<name>` is omitted.

```
blast init [<name>] [--db-url <url>] [--no-test-db] [--force]
```

| Flag | Meaning |
|------|---------|
| `<name>` | Optional. If given, scaffold to `./<name>/`. If omitted, scaffold directly into cwd (must be empty unless `--force`). |
| `--db-url <url>` | Postgres URL for the project. If omitted, prompts interactively. |
| `--no-test-db` | Skip creation of the test database and `.env.test` file. |
| `--force` | Allow scaffolding into a non-empty directory; recreate existing databases. |

Same baked-canonical model as `blast new`. Use `blast init` when you have already `mkdir`'d into the project directory or want in-place initialization.

## `blast build`

Produces a release binary. Implemented in `src/build.rs`.

```
Step 1: blast check              (Governor lint on frontend — currently stubbed; enforced when Governor ships)
Step 2: npm run build            (Vite build to frontend/dist/)
Step 3: cargo build --release    (release binary in target/release/<project>)
```

Frontend build is skipped if `frontend/` does not exist. Errors from any step abort the sequence and surface via `BlastError::Subprocess`.

## `blast package`

Archives the release build into a deployable tarball. Requires a prior `blast build`.

Contents of `release-<name>-<timestamp>.tar.gz`:

| Path | Included if |
|------|-------------|
| `target/release/<project>` | always |
| `frontend/dist/` | exists |
| `.env.example` | exists |
| `deploy/systemd/<project>.service` | exists (scaffolded by `blast new`) |

Placed at project root. Intended for `scp` to VPS + `systemd` reload. Exact deployment steps are documented in the systemd unit template generated by `blast new`.

## `blast test [filter]`

Runs the integration test suite against a real Postgres test DB.

```
Step 1: Create test DB (<appname>_test_<pid> or DATABASE_URL_TEST)
Step 2: Run all migrations on test DB
Step 3: cargo test [-- filter] (with DATABASE_URL_TEST set)
Step 4: Drop test DB (unless --no-drop)
```

See `catalyst/doc/SPEC_TESTING.md` for the full testing strategy.

## `blast gen test`

Scaffolds baseline test files for all generated flows and routes. Idempotent on existing files — does not overwrite tests the user has already modified.

```
blast gen test                       # scaffold every flow + every route from resource state
blast gen test --flow <name>         # only flow tests; <name> is "<table>" or "<table>/<verb>"
blast gen test --route <name>        # only the route smoke test for <table>
```

Outputs:

```
src/flows/generated/<resource>/<verb>.test.rs   (per declared verb)
src/transport/http/generated/<resource>.test.rs (one per resource with routes)
tests/common/fixtures/<resource>.rs             (fixture impl, one per resource)
tests/common/fixtures/mod.rs                    (barrel of fixture modules)
tests/common/mod.rs                              (shared harness; `use canonical::*` rewritten to project name on scaffold)
```

Every emitted file is an **opinionated stub**: imports, `#[tokio::test]` attribute, `run_in_test` wrapper, fixture call, and a placeholder assertion the user replaces.

For each target file:
- If the file does not exist: write the Blast-generated scaffold.
- If the file already exists: skip (do not overwrite). User is responsible for keeping existing test files current.

`blast gen all` runs `blast gen test` as the final pass in the pipeline.

See `catalyst/doc/SPEC_TESTING.md` for what the scaffolds contain and how the catalyst-side harness primitives (`with_test_transaction`, `TestCtx`, `Fixture`, `fixture!`) wire together.

## Legacy Commands (Removed or Superseded)

- `scss`, `css`, `publish-css`, `js`, `cdn` — legacy asset pipeline. Vite handles everything now. Removed.
- `vessel migrate`, `vessel refresh` — legacy migration system. Diesel is the only path. Removed.
- `cronjobs` commands — renamed to `blast fuses`. Removed.
- `blast spark` — plugin system killed. No sparks. Removed.
- `blast gen primer`, `blast gen blueprint` — the DSL sub-crates (`catalyst_primer`, `catalyst_blueprint`) are deleted. State lives in `storage/blast/state/` RON files. Removed.
- `blast gen table`, `blast gen resource` — the standalone wizards are gone. Both are now folded into `blast migration` as one chained ratatui wizard (table SQL + RON state + migrate + gen schema + gen all in a single flow).
- `blast gen migration --custom` — the empty/$EDITOR-driven migration skeleton is gone. Hand-write the SQL file directly if you need raw migrations (views/triggers/etc.); the wizard is for opinionated CRUD tables only.

## Config Source

Blast operating config comes from `storage/blast/state/app.ron` in the user's project. No `Blast.toml`. The `--config-file <path>` flag (planned: deserialize any command's `Args` from a RON/JSON file) supports CI/automation without interactive input.

Behaviors driven by `app.ron`:
- Env var spec (where `blast init` reads expected env vars, drives `.env.example`)
- Default struct derives (applied in codegen unless per-resource override)
- FE lint rules (Governor)
- Admin config, fuses schedule, services config

See `SPEC_STATE.md` for the full `app.ron` schema.

## Logging

Commands output through the injected `Sink` — not through a global logger. The CLI `StdoutSink` impl uses emoji-prefixed stdout internally; the TUI `WidgetSink` routes to ratatui widgets. There is no "dashboard mode" branch that suppresses stdout — that model is gone.

Progress via `indicatif` (CLI) or `WidgetProgress` (TUI), ephemeral — not persisted.

File log at `storage/blast/blast.log` for post-hoc review (`blast log view`).

## Command Core Contract

**One core, two front-ends.** CLI clap parser and TUI menu are *front-ends*. The actual work lives in pure command functions. Front-ends resolve args, then call the core.

### Layering

```
src/cli.rs          ← clap derive → Command::Args
src/wizards/...     ← ratatui wizards → Command::Args (wizards never execute)
src/wizards/widgets ← shared list_select / text_input ratatui primitives
src/commands/       ← pure command core (this is the surface)
  └─ <verb>.rs      ← fn run(args: Args, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome>
```

### Hard rules

1. **Args fully resolved at front-end boundary.** The function signature for every command is `fn run(args: FullyTypedArgs, sink, progress) -> BlastResult<Outcome>`. No `Option<T>` for "ask if missing" — the front-end already asked. Commands take ground truth.

2. **No prompting inside command bodies.** Zero TUI prompt calls under `src/commands/`. Wizards live in `src/wizards/<wizard>/` and *output* a fully-resolved arg struct that gets passed to the core command. The dialoguer dep is gone — all interactive surfaces use the shared ratatui widgets at `src/wizards/widgets/`.

3. **No direct stdout/stderr.** Zero `println!`, `eprintln!`, raw `print!`. Commands emit through an injected `Sink` trait:
   ```rust
   pub trait Sink {
       fn info(&mut self, msg: &str);
       fn warn(&mut self, msg: &str);
       fn error(&mut self, msg: &str);
       fn success(&mut self, msg: &str);
       fn debug(&mut self, msg: &str);
       fn structured(&mut self, event: SinkEvent);  // typed events for TUI widgets
   }
   ```
   CLI front-end uses `StdoutSink` (emoji-prefixed stdout). TUI front-end uses `WidgetSink` (events into a ratatui pane).

4. **Progress via injected `Progress` sink.** Long-running ops (`gen all`, `init`, `build`) emit progress events:
   ```rust
   pub trait Progress {
       fn step_start(&mut self, label: &str);
       fn step_done(&mut self, label: &str);
       fn step_fail(&mut self, label: &str, reason: &str);
       fn tick(&mut self, current: u64, total: u64);
   }
   ```
   CLI uses `IndicatifProgress`. TUI uses `WidgetProgress`. Tests use `NullProgress`.

5. **Outcome is a typed return value.** Each command returns `Outcome` describing what it did (files written, files skipped, errors recovered). Front-ends render the outcome in their idiomatic way.

6. **Command registry is single-source.** `enum Command` lives in `src/commands/mod.rs`. Each variant has:
   - typed args struct
   - clap derive metadata (CLI parses straight into the variant)
   - menu metadata (label, category, description) consumed by TUI for menu rendering
   - `run` fn

   Adding a command = one variant, one args struct, one `run` fn. CLI and TUI pick it up automatically. No four-place sync.

### What this kills

- The `commands.rs` custom parser + `interactive.rs` dispatch + dashboard menu wiring as three separate sites of truth.
- Any command body that pops a TUI prompt directly. Prompts belong in wizards.
- `logger::info(...)` calls inside command bodies (the CLI `Sink` impl does that, not the command).
- Hand-written menu lists in `interactive.rs` and `dashboard.rs` that drift from the CLI surface.
- "Dashboard suppress stdout" branch in the old logger.

## Guidance For Blast Maintainers

- CLI and TUI front-ends call the same command-core `run` fn. Front-ends never duplicate work.
- When adding a command: add a `Command` variant + args struct + `run` fn. Done. Both front-ends pick it up.
- Prefer explicit command names over cleverness. `blast gen flows` beats `blast flux`.
- Deterministic output layouts only. No "depending on time, this ends up here or there."
- Avoid one-off project-specific behavior in core commands. If a project needs something weird, fork.
- Run `cargo check` in `blast/` after touching Blast source.

## Related Specs

- `SPEC_STATE.md` — state file format, schema versioning, atomic write, build.rs safety net
- `SPEC_CODEGEN.md` — what each `blast gen` target emits
- `SPEC_GOVERNOR.md` — `blast check` internals
- `catalyst/doc/SPEC_FUSES.md` — `blast fuses` subcommand semantics
