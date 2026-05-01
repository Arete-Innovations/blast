# CLAUDE.md — agent guide for this Catablast app

This file is the entry point for AI agents working in this codebase. Read it once on session start.

## What this is

A **Catablast** app: opinionated full-stack Rust monolith.

| Layer | Stack |
|-------|-------|
| Backend | Rust + Axum |
| Persistence | Diesel + Postgres |
| Auth | Opaque session tokens (NOT JWT) — table `sessions` |
| Errors | `MeltDown` enum, per-variant HTTP status |
| Retry | `Crank` combinator (flows-only) |
| WebSockets | `Relay` multiplexer (one socket per session) |
| Scheduler | `Fuses` (DB-backed, flow-dispatched) |
| Frontend | Leptos 0.7 SSR + islands hydration (target_arch=wasm32 split, no feature flags) |
| Component lib | native HTML inputs in codegen; thaw available for hand-rolled wasm-only components |
| CSS | scss compiled by cargo-leptos via grass + per-component `.module.scss` via stylance. OKLCH color, semantic tokens. |
| Dev CLI | `blast` (this is what you run) |

Stack is **locked**. Don't propose Sycamore, Yew, Dioxus, web-awesome, shoelace, JWT, Tailwind, Tera, Rocket, etc. If the user wants different they fork.

## The Cardinal Rule: layered architecture

Every `src/<x>/` directory is a layer. Imports cross layers in ONE direction. `build.rs` panics the build on violation.

```
structs/    ← inert types (rows, DTOs, newtypes)
database/   ← pool, migrations, schema.rs (stateful)
services/   ← stateless adapters (crypto, email, http client)
models/     ← persistence — only DB-touching layer above database/
routines/   ← atomic capabilities (one business action; compose models + services)
flows/      ← capability inventory (auth boundary + Crank policy)
transport/  ← thin entry points (http / ws / leptos / fuses)
```

**Hard rules:**

- `transport/` imports `flows`, `structs`. **Nothing else.**
- `flows/` imports `routines`, `structs`, `crank`. **Nothing else.** No flow→flow.
- `routines/` imports `models`, `services`, `structs`, `database` types. No routine→routine. Routines are leaves.
- Every flow declares a `Crank` policy explicitly — `Crank::none()` if no retry.
- `structs/` may import `crate::database::schema` (Diesel `table!` output) for derive macros. That's the only allowed cross-import.

Full graph + lint rule list: `doc/SPEC_ARCHITECTURE.md`.

## Two-tier ownership

Every layer has two kinds of subdirectories:

| Subdir | Owner | Edit policy |
|--------|-------|-------------|
| `<layer>/generated/` | **Blast** | Wiped wholesale on `blast gen`. **Never edit by hand.** |
| `<layer>/<resource>/` | **You** | Hand-written forever. Blast won't touch post-scaffold. |

The scaffolded app ships with hand-written `flows/auth/`, `flows/sessions/`, `models/auth/`, etc. — those are yours.

To override generated behavior: write a hand-rolled file in `<layer>/<resource>/` that calls or replaces generated code, OR remove the resource's primer and write everything by hand.

## The blast CLI — your only dev tool

| Command | Purpose | Needs TTY |
|---------|---------|-----------|
| `blast` | Open Zellij dashboard | yes |
| `blast cli` | Interactive menu (ratatui) | yes |
| `blast migration` | New-table wizard (TUI) | yes |
| `blast dashboard` | Same as bare `blast` | yes |
| `blast new <name>` | Scaffold new project (used to create THIS app) | no |
| `blast migrate` | Run pending migrations | no |
| `blast rollback` | Roll back N migrations | no |
| `blast schema` | Regen `src/database/schema.rs` from live DB | no |
| `blast seed` | Run seed SQL | no |
| `blast gen all` | Full codegen pipeline (Rust only — single crate compiles to host + wasm32) | no |
| `blast gen <target>` | One pass: structs/models/routines/flows/types/api/validators/pages/components/enums | no |
| `cargo leptos build` | Compile SSR binary + wasm bundle | no |
| `cargo leptos serve` | Build + run dev server (autoreload) | no |
| `blast run` | Start dev server daemon | no |
| `blast run-prod` | Start production server | no |
| `blast stop` | Stop dev/prod daemon | no |
| `blast watch` | cargo-leptos watch loop | no (long-running) |
| `blast fuses <sub>` | Manage scheduled jobs | no |
| `blast log` | Tail blast log files | no |
| `blast toggle-env` | Flip dev ↔ prod | no |
| `blast build` | Production cargo-leptos build | no |
| `blast package` | Archive release artifact | no |

`blast <subcmd> --help` for flags.

**Frontend lint = build.rs `LEPTOS:*` family.** No separate frontend lint pass — `cargo check` runs `build.rs` which panics on `LEPTOS:1..4` violations. The old `blast check` / `Governor` engine is deleted.

**Agents (no TTY): use the non-TTY commands. The wizard at `blast migration` requires a terminal — for headless work, do the manual recipe below.**

## Recipe: add a new resource (table)

Two paths.

### Path A — wizard (interactive only)

```
blast migration
```

Opens a TUI wizard. You fill in: table name, columns (name + type + nullable + indexed), verbs (List/Get/Create/Update/Delete), auth mode (Public/AuthRequired/AdminOnly/Roles), gen_level. On Done it emits:

- `storage/blast/state/resources/<name>.ron` — the **Primer** (per-resource state file)
- `migrations/<timestamp>_<name>/up.sql` + `down.sql`
- Runs `diesel migration run`
- Runs `blast gen all`

`cargo check` should pass. Resource is wired end-to-end.

### Path B — manual (works without TTY)

```bash
# 1. Hand-write SQL
mkdir migrations/$(date +%Y-%m-%d-%H%M%S)_<name>
$EDITOR migrations/*_<name>/up.sql      # CREATE TABLE ...
$EDITOR migrations/*_<name>/down.sql    # DROP TABLE ...

# 2. Apply
blast migrate
blast schema                             # refresh src/database/schema.rs

# 3. Author the Primer (RON)
$EDITOR storage/blast/state/resources/<name>.ron
# Full RON shape + worked example: doc/SPEC_PRIMER.md

# 4. Codegen + build
blast gen all
cargo check
cargo leptos build
```

`doc/SPEC_PRIMER.md` is the authoritative reference for the Primer RON shape — every field, every enum variant, full annotated example. Read it before hand-rolling.

## Recipe: modify an existing resource

1. Edit `storage/blast/state/resources/<name>.ron` (the Primer).
2. If schema changed: write a new migration → `blast migrate` → `blast schema`.
3. `blast gen all`.
4. `cargo check` + `cargo leptos build`.

## Logging (cata_log!)

```rust
cata_log!(Debug, "schema_parser: parsed 12 tables");
cata_log!(Info,  "bootstrap: pool ready");
cata_log!(Warning, format!("missing .env: {}", e));
cata_log!(Error, format!("flow failed: {}", err));
cata_log!(Trace, "relay: tick");
```

Five levels: Trace, Debug, Info, Warning, Error. Macro auto-attaches `src.file` + `src.line` via `#[track_caller]`. For typed structured fields, drop to `tracing::info!(user_id = %id, "msg")` directly. **Never `println!` / `eprintln!` in app code** — bypasses subscriber.

Full spec: `doc/SPEC_LOGGING.md`.

## Errors (MeltDown)

```rust
use crate::meltdown::MeltDown;
return Err(MeltDown::NotFound("user".to_string()));
return Err(MeltDown::Validation(vec![field_error]));
```

Per-variant HTTP status mapping is centralized. Logging happens at the call site that constructs the error, NOT inside `MeltDown::into_response`. Don't add logging to the impl.

Full spec: `doc/SPEC_MELTDOWN.md`.

## Backend is the single source of truth (binding)

Everything that can be done on the backend MUST be done on the backend. Zero trust in the client. Zero client-side logic where the backend can provide. This is an architectural rule, not a style preference.

- **Error messages.** The backend owns the wording (and the public-vs-internal split inside `MeltDown`). The frontend renders `error.message` from the BE envelope **byte-for-byte**. No FE-side hardcoded user-facing strings (e.g. `'Registration failed'`, `'Login failed'`). No FE-side fallback strings layered on top of typed envelopes. The only allowed FE fallback is for cases where the BE envelope was unreadable (true network failure, malformed JSON) — and that fallback lives in **one** central place, not duplicated per endpoint.
- **Validation.** Field rules live in the Primer (`storage/blast/state/resources/<name>.ron`). Codegen emits a single Rust validator source that compiles to both the SSR binary and wasm. Same fn called from REST handlers AND from form components. No FE-only or BE-only rules. No drift.
- **Authorization.** Auth/role checks live in flows. The FE may HIDE controls based on role for UX, but it never DECIDES auth. Every protected endpoint is gated server-side; FE rendering is an optimistic projection of what the server will allow.
- **Business logic, formatting, computed values.** Derive in the BE. Never re-implement BE rules in the FE.
- **Typed error envelope.** `MeltDown` defines the wire shape. The wasm `api_client.rs` has ONE typed parser for it (`parse_or_envelope_error`). Hand-rolled `is_xxx_response` shape checks scattered across pages are a smell — codegen (or a single helper) owns deserialization.

If you find yourself adding a hardcoded user-facing string to the FE, a fallback message that hides the BE's actual `message` field, or a re-implementation of a BE rule — **stop and push it back to the BE**.

## What to NEVER touch

- **`<layer>/generated/`** anywhere — wiped on next `blast gen <pass>`. Edits are lost silently.
- **`src/database/schema.rs`** — regenerated by `blast schema` from live DB. Edit migrations.
- **`storage/blast/state/`** — RON state files. Use the wizard or hand-edit; never delete a primer if generated code references it.
- **`.env` load order in `src/main.rs`** — `dotenv::dotenv()` MUST run before `cata_log::init_tracing()`. Reordering silently breaks `RUST_LOG`/`LOG_LEVEL`.

## Deep-dive specs (read on demand)

| Question | Spec |
|----------|------|
| Layer rules / dep graph / build.rs lint | `doc/SPEC_ARCHITECTURE.md` |
| Per-resource Primer RON shape | `doc/SPEC_PRIMER.md` |
| Authoring a flow | `doc/SPEC_FLOWS.md` |
| Error variants + HTTP mapping | `doc/SPEC_MELTDOWN.md` |
| Retry policy authoring | `doc/SPEC_CRANK.md` |
| WebSocket multiplexer + pub/sub | `doc/SPEC_RELAY.md` |
| Scheduled jobs | `doc/SPEC_FUSES.md` |
| Auth tokens / session middleware | `doc/SPEC_SESSIONS.md` |
| Leptos UI / SSR + hydrate / data fetch / forms | `doc/SPEC_LEPTOS.md` |
| CSS tokens, stylance, OKLCH | `doc/SPEC_CSS.md` |
| `cata_log!`, tower span, JSON logs | `doc/SPEC_LOGGING.md` |
| Single-source validator codegen | `doc/SPEC_VALIDATORS.md` |
| Real Postgres + transaction rollback testing | `doc/SPEC_TESTING.md` |
| Stateless adapters | `doc/SPEC_SERVICES.md` |
| RON state, .env layering | `doc/SPEC_CONFIG.md` |
| Generic admin shell | `doc/SPEC_ADMIN.md` |

## Always do before reporting "done"

1. `cargo build` — green
2. `cargo leptos build` — wasm bundle compiles
3. `cargo test` — affected resource's integration tests pass

## Hard noes

- `println!` / `eprintln!` in app code. Use `cata_log!`.
- `JWT`. Sessions are opaque bearer tokens.
- `Sycamore` / `Yew` / `Dioxus` / `web-awesome` / `shoelace` as alternatives. Stack is locked on Leptos 0.7.
- `Tailwind` / utility-class CSS. Use semantic classes in `.module.scss` + `var(--app-*)` tokens.
- inline `style=` in `view!` macros. `LEPTOS:1` rejects it.
- raw hex / rgb / hsl colors outside `style/tokens.scss`. `LEPTOS:2` rejects them.
- raw `px` outside `style/tokens.scss` / `style/base.scss`. `LEPTOS:3` rejects them.
- page components without `<PageShell layout=...>`. `LEPTOS:4` rejects them.
- `unwrap()` / `expect()` in handler code. Map to `MeltDown`.
- Cross-layer imports (`flows/foo` calling `models::*` etc.). build.rs will panic.
- Editing anything inside a `generated/` subdir.
- `#[allow(dead_code)]` / `#[allow(unused_*)]` / `#[allow(unreachable_code)]`. build.rs lint `DEAD:8` rejects them.
- Auto-pushing git commits. Commit locally; let the user push.