# SPEC_FUSES

Flow-dispatched scheduler. DB-backed, typed, registration-based. Replaces what other stacks call "cron jobs."

## Why "Fuses"

A fuse is a timed trigger. When the fuse burns down, it fires. Every registered Fuse has a schedule; when the schedule fires, it dispatches to exactly one flow.

Our scheduler is tightly coupled to flows (typed dispatch, single capability inventory) and DB-backed (status, last-run, last-error persisted). That divergence from generic "cron" earned the themed name.

## DB Table

```sql
CREATE TABLE fuses (
    id              BIGSERIAL PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    flow_name       TEXT NOT NULL,
    schedule_kind   TEXT NOT NULL,
    schedule_spec   TEXT NOT NULL,
    enabled         BOOLEAN NOT NULL DEFAULT true,
    last_run_at     TIMESTAMPTZ,
    last_run_status TEXT,
    last_error      TEXT,
    next_run_at     TIMESTAMPTZ NOT NULL,
    run_count       BIGINT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX fuses_next_run_at_idx ON fuses (next_run_at) WHERE enabled;
```

The table name is `fuses`, reserved for Catalyst. User apps don't create a different table for scheduled work.

## Registration (code)

```rust

use crate::transport::fuses::*;
use std::time::Duration;

pub fn register(registry: &mut FuseRegistry) {
    registry.add(
        Fuse::named("cleanup_expired_sessions")
            .schedule(Schedule::every(Duration::from_minutes(5)))
            .run(crate::flows::sessions::cleanup_expired::run)
    );
}
```

```rust

pub mod custom;

pub fn register_all(registry: &mut FuseRegistry) {
    custom::cleanup_expired_sessions::register(registry);
    custom::rotate_api_keys::register(registry);
    custom::send_daily_digest::register(registry);

}
```

Registry is called at app boot (in `bootstrap.rs`):

```rust

let mut fuse_registry = FuseRegistry::new();
transport::fuses::register_all(&mut fuse_registry);
transport::fuses::launch(pool.clone(), fuse_registry).await;
```

## Schedule Kinds

```rust
Schedule::every(Duration)
Schedule::cron("0 2 * * *")
Schedule::at(chrono::NaiveTime::...)
```

All resolve to `next_run_at` timestamps in the DB row.

## Registry → DB Reconciliation

On app boot, `transport::fuses::launch` reconciles:

1. Read all registered fuses in code (`FuseRegistry`)
2. Read all fuses in DB
3. For each registered fuse:
   - Not in DB → insert
   - In DB but schedule changed → update
   - Missing from code → log warning (don't delete; operator may have disabled it)

**Rule: code is source of truth for existence; DB is source of truth for state (enabled flag, last_run, next_run).**

An operator can disable a Fuse from DB (`UPDATE fuses SET enabled = false WHERE name = 'foo'`) without touching code — Blast's TUI (`blast fuses toggle foo`) flips the flag.

## Runner

One background loop inside the Catalyst process:

```
loop:
    now = Utc::now()
    due = SELECT * FROM fuses WHERE enabled AND next_run_at <= now
    for fuse in due:
        tokio::spawn(run_fuse(fuse))   // concurrent within safety limit
    sleep(poll_interval)   // default 1s
```

`run_fuse`:

1. Mark `last_run_status = 'running'`, `last_run_at = now()`
2. Lookup flow by `flow_name` in a registry (populated at boot)
3. Invoke `flow.run(&ctx)`
4. On success: mark `last_run_status = 'ok'`, bump `run_count`, compute new `next_run_at`
5. On `MeltDown`: mark `last_run_status = 'error'`, persist `last_error`, compute `next_run_at` (same schedule — Fuses don't retry within a run; retries live inside the flow via its declared `Crank` policy)

## Lifecycle Semantics

- **Enabled/disabled toggle:** DB flag. No restart required; runner respects it next poll.
- **Schedule change:** code → `blast gen fuses` (updates registry & DB).
- **Removal:** remove from code; next boot leaves the DB row in place (disabled flag manually if you want it gone).
- **Concurrency:** one run at a time per Fuse. If a run takes longer than the interval, next run waits until the current completes. No overlapping.
- **Missed runs:** if the process was down when `next_run_at` passed, on next startup the Fuse runs immediately (once), then resumes normal schedule. No catch-up bursts.

## Flow Contract

Each Fuse dispatches to a flow. The flow signature:

```rust
pub async fn run(ctx: &Ctx) -> Result<(), MeltDown>;
```

Fuses don't take input. They're triggered by time, not request data. The flow itself reads whatever state it needs (via `ctx.conn()`).

Long retries are acceptable (Fuses aren't user-facing). Every flow dispatched by a Fuse must declare a `Crank` retry policy explicitly — set deadlines generously for background work.

## TUI

`blast fuses`:

```
> blast fuses interactive
[ENABLED ]  cleanup_expired_sessions   every 5m   last: 2026-04-24 14:00, ok
[ENABLED ]  send_daily_digest          cron 0 9 * * *   last: 2026-04-24 09:00, ok
[DISABLED]  rotate_api_keys            every 24h   last: 2026-04-20 14:00, error
```

Commands:
- `blast fuses list` — table output
- `blast fuses toggle <name>` — flip enabled flag
- `blast fuses run <name>` — trigger one immediate run (bypass schedule)
- `blast fuses logs <name>` — show last N runs with errors

## Observability

Every run logs structured events:

```
fuse_run_started { name, attempt }
fuse_run_succeeded { name, duration_ms }
fuse_run_failed { name, duration_ms, error_type, error_message }
```

Per `SPEC_MELTDOWN.md` logging rules, errors bubble through middleware-equivalent fuse logging.

## Anti-Patterns

**Business logic inline in a Fuse handler:**
```rust

Fuse::every(...)
    .run_inline(|ctx| async move {
        sqlx::query("DELETE FROM ...").execute(ctx.conn()).await?;
        Ok(())
    })
```

Fuses dispatch to flows. If you're writing code inline, extract it into a flow file (e.g. `flows/jobs/whatever.rs`) and dispatch to that.

**Using Fuses for request-response:**
Fuses are fire-and-forget background work. HTTP handlers don't wait for Fuse results. If a user action needs a side-effect immediately, do it in the flow for that action.

**Registering the same name twice:**
`Fuse::named("foo")` in two places is a boot-time panic. Names must be unique.

**Fuses that call other Fuses' flows:**
Fine, actually. Flows are reusable regardless of who triggers them. A Fuse-dispatched flow can be the same flow a CLI or HTTP request dispatches. Capability inventory is unified.

## Related Specs

- `SPEC_FLOWS.md` — Fuse-dispatched flow shape
- `SPEC_ARCHITECTURE.md` — `transport/fuses/` layer
- `SPEC_MELTDOWN.md` — error reporting
- `SPEC_CRANK.md` — retry inside Fuse-dispatched flows
- `blast/doc/SPEC_BLAST_COMMANDS.md` — `blast fuses` subcommand surface
