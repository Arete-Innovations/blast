# SPEC_STATE

State files are how the user communicates intent to Blast. They live in the user's app repo, are human-readable, are version-controlled, and are the single source of truth for all Blast codegen.

## Why RON, Why Per-Resource Split

**RON** (Rusty Object Notation) is used because:
- It is Rust-native. Blast already depends on Rust; no extra parser dep.
- It supports comments (unlike JSON). State files benefit from inline documentation.
- It is more readable than TOML for nested structures.
- It round-trips through `serde` without loss.

**Per-resource split** (one file per resource, one for app-wide config):
- Kills merge conflicts. Two developers adding resources never touch the same file.
- Scopes git diffs to the resource that changed.
- Blast's schema upgrader can migrate one file at a time without locking everything.
- Clarity: `storage/blast/state/resources/users.ron` is exactly what it sounds like.

## Directory Layout

```
<user-app>/
└── storage/
    └── blast/
        └── state/
            ├── app.ron                   # app-wide policy
            └── resources/
                ├── users.ron             # per-resource: fields, verbs, ws events
                ├── orders.ron
                └── ...
```

Blast creates `storage/blast/state/` on `blast new` and `blast init`. Resource files are created by `blast gen resource [name]` (TUI wizard) or by hand.

## `app.ron` Schema

```ron
AppState(
    schema_version: 1,

    fe_lint: FeLintConfig(
        max_lines_per_sfc: 600,
        max_lines_per_fn: 120,
        hairline_border_rem: "0.0625rem",
        exempt_color_files: ["src/plugins/primevue.ts"],
        exempt_px_files: [
            "src/plugins/primevue.ts",
            "src/styles/tokens.css",
            "src/styles/base.css",
        ],
        whitelist_snippets: ["schema.org"],
        deny_rules: [
            ConsoleLog,
            InlineStyle,
            RawRemOutsideTokens,
            TypeAny,
            TsIgnore,
            SilentFallback,
            IconClassOutsideIconsFile,
            RawColorOutsidePreset,
            HardcodedPx,
        ],
    ),

    admin: AdminConfig(
        enabled: true,
        mount_path: "/admin",
        actions: [
            AdminAction(slug: "reset-password", label: "Reset password"),
        ],
    ),

    fuses: FusesConfig(
        schedules: {
            "cleanup_sessions": "0 3 * * *",
            "send_digests": "0 8 * * 1",
        },
    ),

    services: ServicesConfig(
        storage: Local(base_path: "storage/uploads"),
        email: Smtp(
            from: "app@example.com",
        ),
        rate_limit: InMemory(
            default_rpm: 120,
        ),
    ),

    nav: NavConfig(
        sections: [
            Section(
                key: "main",
                label: "Main",
                icon: "home",
                entries: [
                    Entry(route: "dashboard"),
                    Entry(route: "users.list", roles: [Admin]),
                    Entry(route: "orders.list"),
                ],
            ),
            Section(
                key: "ops",
                label: "Operations",
                icon: "tools",
                roles: [Admin],
                entries: [
                    Entry(route: "fuses.list"),
                    Entry(route: "audit.list"),
                ],
            ),
        ],
    ),

    pages: [
        Page(
            route: "dashboard",
            path: "/",
            component: "custom/pages/DashboardPage.vue",
            layout: "cards",
            label: "Dashboard",
            icon: "dashboard",
            in_nav: true,
            roles: [User, Admin],
        ),
        Page(
            route: "settings",
            path: "/settings",
            component: "custom/pages/SettingsPage.vue",
            layout: "cards",
            label: "Settings",
            icon: "cog",
            in_nav: true,
            roles: [User, Admin],
        ),
        Page(
            route: "debug.thing",
            path: "/_debug/thing",
            component: "custom/pages/DebugThing.vue",
            layout: "bleed",
            in_nav: false,
            roles: [Admin],
        ),
    ],

    env_spec: [
        EnvVar(key: "DATABASE_URL", required: true, description: "Postgres connection string"),
        EnvVar(key: "SESSION_SIGNING_KEY", required: true, description: "32-byte hex secret"),
        EnvVar(key: "SMTP_HOST", required: false, description: "SMTP server hostname"),
    ],

    default_derives: ["Debug", "Clone", "Serialize", "Deserialize"],
)
```

All keys under `app.ron` are optional except `schema_version`. Missing keys fall back to defaults baked into Blast.

### `nav` and `pages` (FE routing + navigation)

Both feed the FE routing codegen pass — `frontend/src/generated/router/{routes,route-names,install-router-guards}.ts` and `frontend/src/generated/nav/menu.ts`. See `catalyst/doc/SPEC_FRONTEND_ROUTING.md` for the full philosophy.

**`pages: [Page(...)]`** declares custom (non-CRUD) routes:

| Field | Type | Notes |
|-------|------|-------|
| `route` | string | Route name. Becomes part of `RouteName` union in `route-names.ts`. Dot-notation convention (`dashboard`, `audit.detail`). |
| `path` | string | URL path. Supports vue-router param syntax (`/foo/:id`). No trailing slash. |
| `component` | string | Path to hand-written Vue component, relative to `frontend/src/` (e.g. `custom/pages/DashboardPage.vue`). |
| `layout` | enum | `cards` / `split` / `table` / `bleed` / `tabbed`. Drives `<PageShell layout="...">` codegen. |
| `label` | string | Human-readable name (used in nav + breadcrumbs). |
| `icon` | string | Icon registry key (resolves to `IC.<icon>` from `src/icons.ts`). |
| `in_nav` | bool | If false, route is reachable but not auto-included in any menu. |
| `roles` | [Role] | Auth gating. Codegen emits both router-guard check and menu-visibility check. |

CRUD routes for resources are auto-emitted from each Primer file's verbs — they don't need to appear in `pages`.

**`nav: NavConfig(...)`** declares the menu tree:

| Field | Type | Notes |
|-------|------|-------|
| `sections` | [Section] | Top-level menu groups. |
| `Section.key` | string | Stable identifier for active-route highlighting. |
| `Section.label` | string | Group label. |
| `Section.icon` | string | Icon registry key. |
| `Section.roles` | [Role] | Hide entire section for unprivileged users. |
| `Section.entries` | [Entry] | Menu items inside the section. |
| `Entry.route` | string | Route name. **Must exist** in either auto-emitted CRUD routes or `pages` — codegen fails on dangling reference. |
| `Entry.roles` | [Role] | Per-entry visibility (must be subset of route's auth requirement; codegen validates). |

**Drift impossibility**: every `Entry.route` is validated against the resolved route set at codegen time. Renamed routes break codegen, not runtime. There is no manual `ROUTE_TO_KEY` table.

## `resources/<name>.ron` Schema

```ron
ResourceState(
    schema_version: 1,

    table: "users",

    fields: [
        FieldState(
            column: "id",
            variants: [DB, Public],
        ),
        FieldState(
            column: "email",
            variants: [DB, Insertable, Patch, Public, Admin],
            validation: FieldValidation(
                max_len: Some(254),
                pattern: Some("^[^@]+@[^@]+$"),
            ),
        ),
        FieldState(
            column: "password_hash",
            variants: [DB],
        ),
        FieldState(
            column: "role",
            variants: [DB, Public, Admin],
            validation: FieldValidation(
                enum_values: Some(["user", "admin"]),
            ),
        ),
        FieldState(
            column: "created_at",
            variants: [DB, Public],
        ),
    ],

    verbs: [
        VerbState(
            verb: List,
            enabled: true,
            auth: AuthRequired,
            paginated: true,
            filtered_by: ["role"],
        ),
        VerbState(
            verb: Get,
            enabled: true,
            auth: AuthRequired,
        ),
        VerbState(
            verb: Create,
            enabled: true,
            auth: AdminOnly,
        ),
        VerbState(
            verb: Update,
            enabled: true,
            auth: ScopedTo("id"),
        ),
        VerbState(
            verb: Delete,
            enabled: true,
            auth: AdminOnly,
        ),
    ],

    ws_events: [
        WsEventState(
            trigger_columns: ["role"],
            payload: FullPublicRow,
            topic_scope: PerRow,
        ),
    ],
)
```

Fields not listed in `fields` are skipped in codegen (reachable via `sql_query` / custom models). There is no `raw_rust` field — if the TUI cannot express something, the user writes Rust in `src/<layer>/custom/`.

## Schema Versioning

Every state file carries `schema_version: u32` at the top level.

When Blast loads a state file:

1. Check `schema_version` against Blast's known max version.
2. If the file's version < current: run bundled upgraders in sequence (upgrader N→N+1 for each step).
3. Log each migration: `app.ron: migrated schema_version 1 → 2`.
4. Write the upgraded file back atomically (see below).

Upgraders are pure functions: `fn upgrade_v1_to_v2(old: RonValue) -> Result<RonValue, BlastError>`. They never assume the presence of optional fields — they only add or rename.

If `schema_version` is unknown (higher than Blast's max), Blast errors out with an actionable message: "state file schema_version 5 requires Blast >= 2.x; current is 1.x".

## Atomic Write + Content Hashing

Blast never writes state files in place directly. All state writes use the atomic pattern (`crate::state::io`):

1. Serialize to string in memory.
2. Write to a `.tmp` sibling file: `storage/blast/state/resources/users.ron.tmp`.
3. `rename()` the `.tmp` into place (atomic on POSIX; near-atomic on Windows).
4. Compute the **blake3** content hash of the file (`crate::state::content_hash`).
5. Return the hash for use in codegen markers.

Content hash is stored in generated file headers (see `SPEC_CODEGEN.md`). It is computed at write time and at codegen time from the on-disk bytes. Blast does not embed the hash inside the state file itself.

Hash algorithm choice: blake3 (single global dep, faster than SHA-256 on the typical multi-KB state file, hex-encoded for the marker).

## The Two-Step Workflow

**Mutate → Regen is always explicit.** Blast never auto-regens on state file change.

```
1. blast gen resource users   # TUI wizard mutates resources/users.ron
2. blast gen all              # reads state files → rewrites src/*/generated/ + frontend/src/generated/
```

This is intentional. Auto-regen on file save would:
- Run codegen mid-edit (state file partially written).
- Trigger spurious `cargo check` runs.
- Obscure what changed and when.

The explicit `blast gen all` step is fast (seconds for a medium app) and deterministic. Running it twice in a row produces no changes.

## Build.rs Safety Net

The user app's `build.rs` (scaffolded once by `blast new`, then committed) enforces that codegen is current. The template lives in Blast at `src/codegen/build_rs_template_src.rs.tmpl`, runner at `src/codegen/build_rs_template.rs`. It is **self-contained** — no `blast_build` runtime crate, just `std` + `blake3` (the only entry in `[build-dependencies]`).

What `build.rs` does:

1. Emits `cargo:rerun-if-changed=storage/blast/state/` and the same for each watched generated dir.
2. Walks every `.rs` file under `src/{structs,models,flows,transport/http,transport/ws}/generated/`.
3. For each file: parses the header marker (`// AUTO-GENERATED from <path> @ <hex-hash>`), reads the referenced state file, recomputes the blake3 hash, and calls `panic!` on mismatch:

```
build.rs: state file 'storage/blast/state/resources/users.ron' changed since last regen — run 'blast gen all'
  expected hash: a3f9...
  actual hash:   7b21...
  stale file:    src/structs/generated/users.rs
```

Files without a marker (hand-rolled `custom/`, the layer `mod.rs`, etc.) are skipped silently. State files referenced from a marker but missing on disk also fail loudly ("was it deleted? regenerate with 'blast gen all'").

This means `cargo check` / `cargo build` / `cargo test` all hard-fail when state is out of sync. The user literally cannot forget to regen — the compiler refuses.

## No Raw Rust Escape Hatch

State files have no `raw_rust` field. There is no way to inject arbitrary Rust or TypeScript into codegen output via state files.

If the TUI wizard cannot express what the user needs, the user writes Rust in `src/<layer>/custom/`. The generated/custom layer split is the escape hatch. This is by design:

- Keeps state files data-only and machine-readable.
- Prevents Blast from becoming a template engine for arbitrary code.
- Keeps the generated surface auditable.

## Rename Detection and Refusal

When the user renames a resource (e.g. `User` → `Account`) via the TUI, Blast:

1. Greps `src/**/custom/` for the old symbol (`User`, `UserPublic`, `NewUser`, etc.) before writing the updated state file.
2. If old symbols are found, prints them with file:line context and **refuses to write** (or emits a loud warning, depending on flag).
3. User resolves the references manually in `custom/` — then reruns the wizard to confirm.

There is no magic AST patching. Manual resolution keeps `custom/` code intentional and readable. The grep is text-based (conservative: may have false positives for common names). User can override with `--force-rename` after reviewing the list.

## Related Specs

- `SPEC_CODEGEN.md` — how state files drive generated output, hash marker format
- `SPEC_BLAST_COMMANDS.md` — `blast gen resource` wizard, `blast gen all`
- `SPEC_GOVERNOR.md` — `app.ron` fe_lint section drives Governor
- `catalyst/doc/SPEC_ARCHITECTURE.md` — where generated files land
