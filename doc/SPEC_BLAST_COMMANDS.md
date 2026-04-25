# SPEC_BLAST_COMMANDS

Full command surface of the `blast` CLI. TUI flows, dashboard, and interactive menu. Designed so that CLI and dashboard are two interfaces over the same underlying behavior.

## Top-Level Commands

```
blast new <name> [--dev]        # scaffold a new Catablast app
blast init                       # initialize project: deps, migrations, schema, codegen

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

blast spark add <repo>           # install a spark from git
blast spark sync                 # sync all sparks declared in blueprint
blast spark list                 # list installed sparks

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
blast gen primer                  # compile primer sub-crate, emit IR to target/primer/
blast gen primer <resource>       # TUI flow to author/edit primer/src/<resource>.rs, then emit
blast gen blueprint               # compile blueprint sub-crate, emit IR
blast gen structs                 # primer IR + schema → src/structs/generated/
blast gen models                  # primer IR + schema → src/models/generated/
blast gen flows                   # primer IR → src/flows/generated/ + transport/http/generated/ + transport/ws/generated/
blast gen frontend                # primer + blueprint IR → frontend/src/generated/
blast gen env-example             # blueprint IR → .env.example
blast gen governor-plugin         # blueprint fe_lint IR → frontend/scripts/governor-plugin.js + .rule_violations_whitelist
blast gen table [name]            # interactive migration wizard; emits up.sql / down.sql in migrations/
blast gen test                    # primer IR → *.test.rs scaffolds per flow + per route (idempotent on existing files)
blast gen all                     # schema → primer → blueprint → structs → models → flows → frontend → env-example → governor-plugin → test scaffolds
```

## TUI Flows

### `blast gen primer <resource>`

Interactive contract authoring, powered by dialoguer (`FuzzySelect`, `MultiSelect`, `Input`, `Confirm`).

Steps:

1. **Pick table** — if `<resource>` not provided, list tables from `schema.rs`, user picks via `FuzzySelect`. If the resource already has a primer, pre-selections are loaded.

2. **Per field:** multi-select which variants it belongs to (`DB`, `Insertable`, `Patch`, `Public`, `Admin`). Defaults are smart:
   - Primary keys: `DB + Public`
   - `password_hash`, `*_secret`: `DB` only
   - `created_at`, `updated_at`: `DB + Public` (readonly)
   - Everything else: all variants

3. **Per verb (list/get/create/update/delete):** toggle on/off. For each enabled verb, pick auth mode:
   - `public`
   - `auth_required`
   - `admin_only`
   - `scoped_to:<field>` — dialoguer shows available field names
   - `roles:[...]` — multi-select from known role enum variants

4. **List-specific:** toggle `.paginated()`, multi-select filterable columns for `.filtered_by([...])`.

5. **WebSocket events:** toggle, pick trigger columns, pick payload shape (`FullPublicRow` or `IdOnly`), pick topic scope.

6. **Confirm:** show Rust code preview, confirm → write `primer/src/<resource>.rs`, update `primer/src/lib.rs` re-exports, emit IR.

7. **Power-user bypass:** user can press `e` during confirm to drop into `$EDITOR` and edit the Rust directly.

### `blast gen` (no args)

Launches the dialoguer menu with all codegen targets as selectable items. User picks → runs that target (which may itself have a TUI, like `primer <resource>`).

## Dashboard (`blast dashboard`)

Zellij layout (`storage/blast/dashboard.kdl`):

- **Top pane:** status line (env, last command result, log file tail)
- **Main pane:** interactive menu (same as `blast cli` embedded)
- **Side pane:** live log viewer (ratatui-based, `storage/blast/blast.log`)
- **Bottom pane:** fuses live table (`blast fuses` auto-refresh view)

Process: runs `zellij --layout ...kdl`, replaces current process. Writes to log file instead of stdout since stdout is captured by Zellij panes.

Dashboard is a presentation layer. All actual work goes through the same `execute()` path as CLI.

## Interactive CLI (`blast cli`)

Simpler alternative to dashboard. `FuzzySelect` menu:

```
> blast cli
? Select a Blast command:
❯ [GEN]      Generate all (full pipeline)
  [GEN]      Generate primer (interactive)
  [GEN]      Generate blueprint
  [GEN]      Generate flows
  [DB]       Run migrations
  [DB]       Rollback migration
  [DB]       Seed data
  [SERVER]   Run dev
  [SERVER]   Run prod
  [FUSES]    Manage fuses
  [LINT]     Run blast check
  [SPARK]    Add spark
  [UTIL]     Toggle env
  [UTIL]     Refresh project
  [EXIT]     Kill session
```

Selection drops into that command's handler (possibly another TUI, possibly a direct execution with log output).

## `blast init` Pipeline

For a freshly-cloned or newly-created project:

```
Step 1/8: Dependency check               (cargo, diesel_cli, node, zellij available?)
Step 2/8: Database: rollback + migrate   (idempotent reset)
Step 3/8: Seed data (if seed file)
Step 4/8: Schema generation              (blast gen schema)
Step 5/8: Primer IR emission             (blast gen primer)
Step 6/8: Blueprint IR emission          (blast gen blueprint)
Step 7/8: Codegen pipeline               (structs → models → flows → frontend)
Step 8/8: Governor plugin + env-example  (blast gen governor-plugin, env-example)
```

Retries critical steps (schema, primer compile, codegen) up to 3 times on failure before exiting.

Progress shown via `indicatif` progress bars. File log at `storage/blast/init.log`.

## `blast new <name>`

Scaffolds a fresh Catablast app:

```
Step 1/6: Clone template repo (GitHub/GitLab/Bitbucket fallback)
Step 2/6: Rename temp dir to <name>
Step 3/6: Set Cargo.toml package name
Step 4/6: Write .env from .env.example template with generated SESSION_SIGNING_KEY
Step 5/6: Initialize primer/ and blueprint/ sub-crates with starter content
Step 6/6: Write initial schema.rs stub + CLAUDE.md scaffold
```

NOT done by `blast new`:
- Create database (user does it; template has DATABASE_URL to point at)
- Run initial migrations (`blast init` does that as a second step)
- Install node deps (user runs `npm install`)

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

After the wizard exits, run `blast migrate` → `blast gen schema` → `blast gen primer <name>` to complete the new-resource flow. See `catalyst/doc/SPEC_PRIMER.md` for the full order of operations.

## `blast gen test`

Scaffolds baseline test files for all generated flows and routes. Idempotent on existing files — does not overwrite tests the user has already modified.

```
blast gen test                       # scaffold every flow + every route from primer IR
blast gen test --flow <name>         # only flow tests; <name> is "<table>" or "<table>/<verb>"
blast gen test --route <name>        # only the route smoke test for <table>
```

Outputs:

```
src/flows/generated/<resource>/<verb>.test.rs   (per primer-declared verb)
src/transport/http/generated/<resource>.test.rs (one per resource with routes)
tests/fixtures/<resource>.rs                    (fixture impl, one per resource)
tests/fixtures/mod.rs                            (barrel of fixture modules)
tests/common/mod.rs                              (shared `use catalyst::testing::*` + test_pool helper)
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
- `cronjobs` commands — renamed to `blast fuses`. The `cronjobs` subcommand and `src/cronjobs.rs` / `src/cronjobs_tui.rs` are removed; use `blast fuses` and its subcommands.

## Config Source

Blast itself is configured almost entirely by the user's blueprint. No `Blast.toml`. Behaviors influenced:

- Env var spec (where `blast init` reads expected env vars)
- Default struct derives (applied in codegen unless per-primer override)
- FE lint rules (Governor)
- Sparks list

Blast reads `target/blueprint/*.json` as its own operating config.

## Logging

```rust
logger::info("...");       // ℹ️ (cli) / file (dashboard)
logger::warning("...");    // ⚠️ (cli) / file
logger::error("...");      // ❌ (cli) / file
logger::success("...");    // ✅ (cli) / file
logger::debug("...");      // 🔍 (cli, verbose only) / file
```

Dashboard mode routes logger output to file only (stdout captured by Zellij panes).

Progress bars via `indicatif`, ephemeral — not logged.

## Command Core Contract

**One core, two front-ends.** CLI clap parser and TUI menu are *front-ends*. The actual work lives in pure command functions. Front-ends resolve args, then call the core.

### Layering

```
src/cli.rs          ← clap parser → Command::Args
src/tui/...         ← ratatui/dialoguer → Command::Args
src/commands/       ← pure command core (this is the surface)
  └─ <verb>.rs      ← fn run(args: Args, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome>
```

### Hard rules

1. **Args fully resolved at front-end boundary.** The function signature for every command is `fn run(args: FullyTypedArgs, sink, progress) -> BlastResult<Outcome>`. No `Option<T>` for "ask if missing" — the front-end already asked. Commands take ground truth.

2. **No prompting inside command bodies.** Zero `dialoguer::*` calls under `src/commands/`. If a wizard exists (`gen table`, `gen primer`), it lives in `src/tui/<wizard>/` and *outputs* a fully-resolved arg struct that gets passed to the core command.

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
   CLI front-end uses `StdoutSink` (current emoji-prefixed stdout). TUI front-end uses `WidgetSink` (events into a ratatui pane).

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

- The `commands::execute()` switch + `interactive.rs` dispatch + dashboard menu wiring as three separate sites of truth.
- Any command body that does `dialoguer::FuzzySelect::new()...interact()`.
- `logger::info(...)` calls inside command bodies (logger becomes the CLI sink impl, not a global).
- Hand-written menu lists in `interactive.rs` and `dashboard.rs` that drift from the CLI surface.

## Guidance For Blast Maintainers

- CLI and TUI front-ends call the same command-core `run` fn. Front-ends never duplicate work.
- When adding a command: add a `Command` variant + args struct + `run` fn. Done. Both front-ends pick it up.
- Prefer explicit command names over cleverness. `blast gen flows` beats `blast flux`.
- Deterministic output layouts only. No "depending on time, this ends up here or there."
- Avoid one-off project-specific behavior in core commands. If a project needs something weird, fork.
- Run `cargo check` in `blast/` after touching Blast source.

## Related Specs

- `SPEC_CODEGEN.md` — what each `blast gen` target emits
- `SPEC_GOVERNOR.md` — `blast check` internals
- `catalyst/doc/SPEC_PRIMER.md` — TUI flow produces primer files
- `catalyst/doc/SPEC_BLUEPRINT.md` — blueprint the TUI may also author
- `catalyst/doc/SPEC_FUSES.md` — `blast fuses` subcommand semantics
