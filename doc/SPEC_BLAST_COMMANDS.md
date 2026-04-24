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
blast gen all                     # schema → primer → blueprint → structs → models → flows → frontend → env-example → governor-plugin
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

## Legacy Commands (To Deprecate)

Currently present in the Blast codebase but flagged for removal or rework:

- `scss`, `css`, `publish-css`, `js`, `cdn` — legacy asset pipeline. Vite handles everything now. Remove when migrations away from HTMX/MaterializeCSS is complete.
- `vessel migrate`, `vessel refresh` — legacy migration system. Diesel is the only path now. Remove.
- `cronjobs` commands — superseded by `fuses`. Keep as alias during transition, remove later.

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

## Guidance For Blast Maintainers

- CLI and dashboard must call the same `execute()` function for each command. No feature drift.
- When adding a new command: add to `commands::Command` enum, `parse_cli_args()`, `execute()`, dialoguer menu in `interactive.rs`. All four.
- Prefer explicit command names over cleverness. `blast gen flows` beats `blast flux`.
- Deterministic output layouts only. No "depending on time, this ends up here or there."
- Avoid one-off project-specific behavior in core commands. If a project needs something weird, it's a Spark.
- Run `cargo check` in `blast/` after touching Blast source.

## Related Specs

- `SPEC_CODEGEN.md` — what each `blast gen` target emits
- `SPEC_GOVERNOR.md` — `blast check` internals
- `catalyst/doc/SPEC_PRIMER.md` — TUI flow produces primer files
- `catalyst/doc/SPEC_BLUEPRINT.md` — blueprint the TUI may also author
- `catalyst/doc/SPEC_FUSES.md` — `blast fuses` subcommand semantics
