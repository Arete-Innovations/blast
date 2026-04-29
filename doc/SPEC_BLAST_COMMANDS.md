# SPEC_BLAST_COMMANDS

Full command surface of the `blast` CLI. TUI flows, dashboard, and interactive menu. Designed so that CLI and dashboard are two surfaces over the same underlying behavior.

## Top-Level Commands

```
blast new <name>                 # scaffold a new Catablast app from vendored canonical
blast init [<name>]              # in-place scaffolder (cwd or <name>/)

blast migration [name]           # create a new Diesel migration skeleton
blast migrate                    # run pending migrations
blast rollback [--steps N]       # roll back N migrations (default 1)
blast seed [file]                # run seed SQL file
blast schema                     # regenerate src/database/schema.rs from DB

blast gen                        # interactive TUI picker
blast gen <target>               # see targets below
blast gen all                    # full pipeline

blast check                      # run Governor lint on frontend

blast run                        # dev server (backend + Vite HMR proxy)
blast run-prod                   # production server (backend only, serving dist/)
blast stop                       # kill background blast run process
blast watch                      # cargo-watch on backend

blast dashboard                  # Zellij-based TUI dashboard
blast cli                        # dialoguer FuzzySelect menu

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
blast gen frontend                # resource state + app state → frontend/src/generated/
blast gen env-example             # app state env spec → .env.example
blast gen governor-plugin         # app state fe_lint section → frontend/scripts/governor-plugin.js + .rule_violations_whitelist
blast gen fe-scaffold             # seed tokens.css, base.css, primevue.ts (idempotent — first-run seed)
blast gen table [name]            # interactive migration wizard; emits up.sql / down.sql in migrations/
blast gen migration [--custom] <name>  # empty migration scaffold (custom = hand-written SQL: views/triggers/etc.)
blast gen resource [name]         # TUI wizard to author/edit storage/blast/state/resources/<name>.ron
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
9.  theme codegen                (codegen::theme::run — emits tokens.css + primevue.ts from app.ron theme section)
10. icons codegen                (codegen::icons::run — emits icons.ts from app.ron icons section)
11. .env.example generation     (codegen::env_example::run)
12. governor plugin emission    (codegen::governor_plugin::run)
```

(Composables, vue components, crud pages, router, ws topics, test scaffold are opt-in via dedicated `blast gen <subcmd>` invocations once their pipeline slots land — see backlog.)

Steps short-circuit cleanly when zero resource state files are declared (logged as "no resources declared; skipping"). Routines + flows additionally filter by `gen_level >= GenLevel::Route`.

Implementation lives in `src/commands/gen_all.rs` as `pub fn run(args, config, sink, progress) -> BlastResult<Outcome>`. `Outcome` carries cumulative `steps_run`, `files_written`, `files_skipped`.

## TUI Flows

### `blast gen resource [name]`

Interactive resource state authoring, powered by dialoguer (`FuzzySelect`, `MultiSelect`, `Input`, `Confirm`). Produces or updates `storage/blast/state/resources/<name>.ron`. Does not run codegen — user runs `blast gen all` after.

The wizard is implemented as **a wizard, not a command**: it lives under `src/wizards/gen_resource/` and produces a fully-resolved `Args` struct that gets handed to the same `run` fn as the CLI. Wizards never execute work themselves — they only resolve arguments.

Steps (each step is a sub-module in `src/wizards/gen_resource/`):

1. **`pick`** — if `[name]` not provided, list tables from `schema.rs`, user picks via `FuzzySelect`. If the resource already has a state file, it's loaded as the seed.

2. **`schema_diff`** (only when editing an existing resource) — compares the on-disk `schema.rs` columns against the resource's stored fields and renders a three-section drift report:
   - `+` columns present in `schema.rs` but missing from state (added)
   - `-` columns present in state but missing from `schema.rs` (removed)
   - `~` columns whose `sql_type` differs (type-changed)

   When **added** columns are present, the wizard prompts to apply them automatically with smart-default variants. Removed/type-changed are surfaced as warnings only — the user resolves them by re-running the wizard's field/verb steps or by editing the state file directly. No silent migrations.

3. **`fields`** — per field: multi-select which variants it belongs to (`Db`, `Insertable`, `Patch`, `Public`, `Admin`). Defaults are smart:
   - Primary keys: `Db + Public`
   - `password_hash`, `*_secret`: `Db` only
   - `created_at`, `updated_at`: `Db + Public` (readonly)
   - Everything else: all variants

4. **`verbs`** — per verb (list/get/create/update/delete): toggle on/off. For each enabled verb, pick auth mode:
   - `public`
   - `auth_required`
   - `admin_only`
   - `scoped_to:<field>` — dialoguer shows available field names
   - `roles:[...]` — multi-select from known role enum variants

5. **`list`** — list-specific: toggle `.paginated()`, multi-select filterable columns.

6. **`ws`** — WebSocket events: toggle, pick trigger columns, pick payload shape (`FullPublicRow` or `IdOnly`), pick topic scope.

7. **`confirm`** — show state file preview, confirm → return `WriteAction::{Created,Updated,Cancelled}`.

8. **Atomic write** — on confirm, `state::save_resource` writes `storage/blast/state/resources/<name>.ron` via the atomic `.tmp` + rename pattern (see `SPEC_STATE.md`).

There is no `raw_rust` field in state files. If the TUI can't express something, the user writes Rust at the top of `src/<layer>/<resource>/` (anywhere outside `<layer>/generated/`). The two-tier user-owned/generated split is the escape hatch.

### `blast gen` (no args)

Launches a dialoguer `Select` menu (`src/gen_picker.rs`) listing every `GenCmd` variant. User picks → the matched variant runs through the same `commands::execute` dispatch as the CLI. `gen resource` and `gen table` then enter their own dialoguer wizards.

## Dashboard (`blast dashboard`)

Zellij layout (`storage/blast/dashboard.kdl`):

- **Top pane:** status line (env, last command result, log file tail)
- **Main pane:** interactive menu (same as `blast cli` embedded)
- **Side pane:** live log viewer (ratatui-based, `storage/blast/blast.log`)
- **Bottom pane:** fuses live table (`blast fuses` auto-refresh view)

Process: runs `zellij --layout ...kdl`, replaces current process. The panes are normal `blast` subprocesses. There is no special "dashboard mode" inside command bodies. The old `logger.rs` "dashboard suppress stdout" branch is dead — commands output through the injected `Sink`, not via a global logger with a dashboard gate.

Dashboard is a presentation layer. All actual work goes through the same `run()` path as CLI.

## Interactive CLI (`blast cli`)

Simpler alternative to dashboard. `FuzzySelect` menu backed by the `Command` registry:

```
> blast cli
? Select a Blast command:
❯ [GEN]      Generate all (full pipeline)
  [GEN]      Generate resource (interactive)
  [GEN]      Generate flows
  [GEN]      Generate frontend
  [DB]       Run migrations
  [DB]       Rollback migration
  [DB]       Seed data
  [SERVER]   Run dev
  [SERVER]   Run prod
  [FUSES]    Manage fuses
  [LINT]     Run blast check
  [UTIL]     Toggle env
  [UTIL]     Refresh project
  [EXIT]     Kill session
```

Selection drops into that command's handler (possibly another TUI, possibly a direct execution with log output).

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

Scaffolds a fresh Catablast app from the **vendored canonical snapshot** baked into the Blast binary at compile time via `include_dir!` from `blast/templates/canonical/`. Does not clone a remote repo. Does not resolve a `catalyst` Cargo dep — there is none. Each scaffolded app is a complete, self-contained framework copy.

```
blast new <name> [--db-url <url>] [--no-test-db] [--force]
```

| Flag | Meaning |
|------|---------|
| `--db-url <url>` | Postgres URL for the new project. If omitted, prompts interactively. |
| `--no-test-db` | Skip creation of the `<dbname>_test` database and `.env.test` file. |
| `--force` | Drop and recreate target databases if they already contain tables. |

Scaffold walks the vendored tree, substitutes `{{project_name}}` in both file paths and file bodies, and writes everything to `./<name>/`.

Total emit: ~52 files. Full implementation in `src/project/{mod, scaffold, templates}.rs`.

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

## `blast gen table [name]`

Interactive migration wizard. Emits a Diesel migration (`up.sql` / `down.sql`) in `migrations/`. Does not apply; user runs `blast migrate` after.

The wizard covers the 80% case: common column types, standard `NOT NULL` / `DEFAULT` choices, single-table FKs, standard indexes. Escape hatch: at the confirm step, user can drop into `$EDITOR` to edit the raw SQL before the file is written.

Steps:

1. **Table name** — prompted if `[name]` not provided. Must be `snake_case` plural. Blast warns on casing violations.
2. **Columns** — interactive loop: column name → type (FuzzySelect from common types: `text`, `varchar(n)`, `integer`, `bigint`, `boolean`, `timestamp`, `timestamptz`, `uuid`, `jsonb`, `numeric`) → nullability → default. Repeat until done.
3. **Primary key** — auto-added `id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY` unless user opts for a custom PK.
4. **Timestamps** — opt-in to auto-add `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()` and `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`.
5. **Foreign keys** — opt-in to add `REFERENCES <table>(id)` on any column. Only tables present in `schema.rs` are offered.
6. **Indexes** — opt-in to add standard single-column indexes.
7. **Preview** — show generated `up.sql` and `down.sql`. Confirm or edit.
8. **Write** — emits `migrations/<timestamp>_<name>/up.sql` and `down.sql`.

`down.sql` is `DROP TABLE IF EXISTS <name>;` by default.

After the wizard exits, run `blast migrate` → `blast gen schema` → `blast gen resource <name>` to complete the new-resource flow.

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
src/tui/...         ← dialoguer wizards → Command::Args (wizards never execute)
src/commands/       ← pure command core (this is the surface)
  └─ <verb>.rs      ← fn run(args: Args, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome>
```

### Hard rules

1. **Args fully resolved at front-end boundary.** The function signature for every command is `fn run(args: FullyTypedArgs, sink, progress) -> BlastResult<Outcome>`. No `Option<T>` for "ask if missing" — the front-end already asked. Commands take ground truth.

2. **No prompting inside command bodies.** Zero `dialoguer::*` calls under `src/commands/`. Wizards live in `src/tui/<wizard>/` and *output* a fully-resolved arg struct that gets passed to the core command.

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
- Any command body that does `dialoguer::FuzzySelect::new()...interact()`.
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
