# SPEC_PRIMER

The **Primer** is the per-resource RON state file that drives codegen. One file per resource, lives at `storage/blast/state/resources/<name>.ron`. Owned by Blast — the new-table wizard writes it, `blast gen` reads it. Hand-editable.

This spec documents the on-disk RON shape so an agent can author one directly without running the wizard.

## File location

```
storage/blast/state/resources/<resource_name>.ron
```

`<resource_name>` is `snake_case`, matches the SQL table name, no plural inflection until codegen (singularization is automatic via inflector — override below if needed).

## Top-level shape

```ron
ResourceState(
    schema_version: 2,
    name: "books",
    fields: { /* IndexMap<FieldName, FieldState> */ },
    verbs:  { /* IndexMap<Verb, VerbState> */ },
    ws_events: None,
    singular_override: None,
    soft_delete: None,
    relations: {},
    gen_level: Composables,
)
```

Authoritative Rust type: `blast::state::resource::ResourceState` (`blast/src/state/resource.rs`).

| Field | Required | Default | Notes |
|-------|----------|---------|-------|
| `schema_version` | yes | — | Always `2`. `RESOURCE_SCHEMA_VERSION` const. Bumped on breaking changes; loader runs upgraders. |
| `name` | yes | — | Snake-case table name. Matches SQL `CREATE TABLE <name>`. |
| `fields` | yes | — | `IndexMap<FieldName, FieldState>`. **Order is preserved** in serialization — used to drive struct field order in some codegen passes. |
| `verbs` | yes | — | `IndexMap<Verb, VerbState>`. Each declared verb generates a flow + transport route. |
| `ws_events` | no | `None` | Pub/sub config for `Relay`. |
| `singular_override` | no | `None` | String. Overrides inflector default singularization. E.g. `data` → `Datum`. |
| `soft_delete` | no | `None` | Soft-delete column config. |
| `relations` | no | `{}` | `BTreeMap<String, Relation>`. Named FK relations to other tables. |
| `gen_level` | no | `Composables` | Codegen depth ceiling. |

## Fields (`FieldState`)

```ron
fields: {
    "id": FieldState(
        sql_type: "Int8",
        variants: [Db, Public, Admin],
        nullable: false,
        primary_key: true,
        validators: [],
    ),
    "title": FieldState(
        sql_type: "Text",
        variants: [Db, Insertable, Patch, Public, Admin],
        nullable: false,
        primary_key: false,
        validators: [Required, MaxLen(200)],
    ),
}
```

| Field | Required | Default | Notes |
|-------|----------|---------|-------|
| `sql_type` | yes | — | String. See SQL type table below. |
| `variants` | yes | — | `BTreeSet<FieldVariant>`. Which projection structs include this field. |
| `nullable` | no | `false` | Maps to `Option<T>` in Rust, `null \| undefined` in TS. |
| `primary_key` | no | `false` | Exactly one PK field per resource expected; codegen relies on it for routes (`/<r>/:id`). |
| `validators` | no | `[]` | `BTreeSet<ValidatorRule>`. Emitted into Rust + TS validator pairs. |

### SQL type strings

The `sql_type` value is a Diesel sql-type tag (string). These are the well-supported values:

| `sql_type` | Postgres | Rust | TS |
|------------|----------|------|----|
| `"Text"` | `TEXT` | `String` | `string` |
| `"Varchar"` | `VARCHAR(N)` | `String` | `string` |
| `"Int4"` | `INTEGER` | `i32` | `number` |
| `"Int8"` | `BIGINT` | `i64` | `number` |
| `"Bool"` | `BOOLEAN` | `bool` | `boolean` |
| `"Timestamptz"` | `TIMESTAMPTZ` | `chrono::DateTime<Utc>` | `string` (ISO-8601) |
| `"Uuid"` | `UUID` | `uuid::Uuid` | `string` |
| `"Jsonb"` | `JSONB` | `serde_json::Value` | `unknown` |
| `"Numeric"` | `NUMERIC` | `bigdecimal::BigDecimal` | `string` |
| `"Enum"` | custom enum | generated Rust enum | string-literal union |

For Postgres `ENUM` columns: `sql_type` is the literal string `"Enum"`. Codegen scans `migrations/**/up.sql` for `CREATE TYPE <x> AS ENUM (...)` statements and matches by column → enum-type-name binding. See `SPEC_VALIDATORS.md` and the Pg ENUM pipeline in CLAUDE.md.

For foreign keys: type is `"Int8"` (the FK column itself). Declare the relation under `relations` (below).

The `SqlType` Rust type is a transparent newtype around `String` — any string parses, but unknown values fail at codegen time. Stick to the table.

### FieldVariant — what gets emitted

Each variant flag controls which generated projection struct includes the field:

| Variant | Emits to | Used for |
|---------|----------|----------|
| `Db` | `<Type>` (`Queryable` row) | DB read result; mirrors table 1:1 |
| `Insertable` | `<Type>Insertable` | POST body for `Create` |
| `Patch` | `<Type>Patch` | PATCH body for `Update` (all fields wrapped in `Option<>`) |
| `Public` | `<Type>Public` | Response body for unauthenticated / public reads |
| `Admin` | `<Type>Admin` | Response body for admin reads (includes hidden columns) |

**Common patterns:**

- `id` (PK): `[Db, Public, Admin]` — read-only, never user-input
- `created_at` / `updated_at`: `[Db, Public, Admin]` — auto-managed
- `deleted_at` (soft-delete): `[Db, Admin]` — admin-only visibility
- User-input column (e.g. `title`): `[Db, Insertable, Patch, Public, Admin]`
- Sensitive field (e.g. `password_hash`): `[Db, Insertable, Patch, Admin]` — never `Public`

### Validators

```ron
validators: [Required, MinLen(3), MaxLen(200), Email]
```

Authoritative enum: `ValidatorRule` (`blast/src/state/resource.rs`).

| Variant | RON | Effect |
|---------|-----|--------|
| `Required` | `Required` | Non-empty (string) / Some (Option) |
| `MinLen(N)` | `MinLen(3)` | String/array length ≥ N |
| `MaxLen(N)` | `MaxLen(200)` | String/array length ≤ N |
| `MinValue(N)` | `MinValue(0)` | Numeric ≥ N (i64) |
| `MaxValue(N)` | `MaxValue(99)` | Numeric ≤ N (i64) |
| `Pattern(s)` | `Pattern("^[a-z]+$")` | Regex match. **Same string fed to Rust + TS** — must be PCRE-portable. |
| `OneOf([...])` | `OneOf(["a", "b"])` | Value in set |
| `Email` | `Email` | RFC-light email regex |
| `Url` | `Url` | URL parse |

Validators emit paired Rust + TS validators with byte-identical regex. See `SPEC_VALIDATORS.md`.

## Verbs (`VerbState`)

```ron
verbs: {
    List: VerbState(
        auth: Public,
        list_options: Some(ListOptions(
            paginated: true,
            filterable_columns: { "title": IlikeContains, "published": Bool },
            sortable_columns: ["created_at", "title"],
            default_sort: Some("created_at"),
            max_page_size: Some(100),
        )),
    ),
    Get:    VerbState(auth: Public, list_options: None),
    Create: VerbState(auth: AuthRequired, list_options: None),
    Update: VerbState(auth: AdminOnly, list_options: None),
    Delete: VerbState(auth: Roles(["admin", "moderator"]), list_options: None),
}
```

Five verbs, all optional — declare only the ones you want. Each emits:

| Verb | HTTP | Flow | Routine | Body type | Response |
|------|------|------|---------|-----------|----------|
| `List` | `GET /<r>` | `flows::generated::<r>::list` | `routines::generated::<r>::list` | — | `ListResponse<<Type>Public>` |
| `Get` | `GET /<r>/:id` | `...get` | `...get` | — | `<Type>Public` |
| `Create` | `POST /<r>` | `...create` | `...create` | `<Type>Insertable` | `<Type>Public` |
| `Update` | `PATCH /<r>/:id` | `...update` | `...update` | `<Type>Patch` | `<Type>Public` |
| `Delete` | `DELETE /<r>/:id` | `...delete` | `...delete` | — | `()` |

`list_options` is required on `List` if you want pagination/filter/sort, else `None`.

### AuthMode

```ron
auth: Public                          // no auth check
auth: AuthRequired                    // any valid session
auth: AdminOnly                       // role == "admin"
auth: ScopedTo("user_id")             // session.user_id matches resource.<col>
auth: Roles(["admin", "moderator"])   // any role in set
```

Maps to flow auth gates:

| AuthMode | Flow code |
|----------|-----------|
| `Public` | (no check) |
| `AuthRequired` | `ctx.require_session()` |
| `AdminOnly` | `ctx.require_role(Role::Admin)` |
| `Roles(set)` | `ctx.require_any(&[Role::X, ...])` |
| `ScopedTo(field)` | `ctx.require_session()` + row predicate `<field> = session.user_id` |

### ListOptions

```ron
list_options: Some(ListOptions(
    paginated: true,
    filterable_columns: {
        "title":     IlikeContains,
        "author_id": Eq,
        "price":     Range,
        "tags":      In,
        "published": Bool,
    },
    sortable_columns: ["created_at", "title", "price"],
    default_sort: Some("created_at"),
    max_page_size: Some(100),
)),
```

| Field | Type | Notes |
|-------|------|-------|
| `paginated` | bool | If `true`, response is `ListResponse<T>` with `?page&page_size`. If `false`, returns `Vec<T>`. |
| `filterable_columns` | `BTreeMap<FieldName, FilterKind>` | Each emits `?filter[col]=val` query param. |
| `sortable_columns` | `BTreeSet<FieldName>` | Allowed values for `?sort=±col`. |
| `default_sort` | `Option<FieldName>` | Applied when no `?sort=` provided. |
| `max_page_size` | `Option<u32>` | Caps `?page_size`. |

**FilterKind values:**

| Variant | SQL | Wire |
|---------|-----|------|
| `Eq` | `col = $1` | `?filter[col]=val` |
| `Range` | `col BETWEEN $1 AND $2` | `?filter[col][from]=...&filter[col][to]=...` |
| `IlikeContains` | `col ILIKE '%' \|\| $1 \|\| '%'` | `?filter[col]=substring` |
| `In` | `col = ANY($1)` | `?filter[col][]=a&filter[col][]=b` |
| `Bool` | `col = $1` | `?filter[col]=true` |

## WebSocket events (`WsEventsState`)

```ron
ws_events: Some(WsEventsState(
    trigger_columns: ["title", "published"],
    payload_shape: Public,
    topic_scope: PerRow,
)),
```

| Field | Notes |
|-------|-------|
| `trigger_columns` | Columns whose change emits a `Relay` event. Empty = any column changes the row. |
| `payload_shape` | `Public` / `Admin` / `IdOnly` — which projection ships in the event payload. |
| `topic_scope` | `Global` (broadcast all) / `PerRow` (`<r>:<id>`) / `ScopedTo(field)` (e.g. `"user_id"` → `<r>:user:<user_id>`). |

See `SPEC_RELAY.md` for transport.

## Soft delete (`SoftDeleteConfig`)

```ron
soft_delete: Some(SoftDeleteConfig(
    column: "deleted_at",
    default_behavior: ExcludeDeleted,
)),
```

The named column must also exist in `fields` (typically `Int8` or `Timestamptz`, nullable, variants `[Db, Admin]`).

| `default_behavior` | Effect on `List` / `Get` |
|--------------------|--------------------------|
| `ExcludeDeleted` | Filter `WHERE <column> IS NULL` unless caller opts in (`?include_deleted=true`). |
| `IncludeDeleted` | Return all rows; caller opts out (`?exclude_deleted=true`). |

When set, `Delete` verb generates an `UPDATE` setting `<column> = NOW()` instead of issuing `DELETE`.

## Relations

```ron
relations: {
    "author":   BelongsTo(table: "users",   fk_local_field: "author_id"),
    "comments": HasMany(table: "comments", fk_remote_field: "book_id"),
}
```

| Variant | Meaning |
|---------|---------|
| `BelongsTo` | This resource carries the FK in `fk_local_field` → `<table>.id` |
| `HasMany` | The other `table` carries the FK in `fk_remote_field` → this resource's id |

Many-to-many is intentionally not modeled in v2. Author the join table as a separate resource.

The named relation drives codegen of loader functions (`load_author(book) -> User`, `load_comments(book) -> Vec<Comment>`).

## GenLevel — codegen depth

```ron
gen_level: Composables
```

| Level | Includes | Emits |
|-------|----------|-------|
| `Struct` | — | `src/structs/generated/<r>.rs` only (data shape) |
| `Model` | + above | `src/models/generated/<r>.rs` (Diesel CRUD) |
| `Route` | + above | `src/routines/generated/<r>/`, `src/flows/generated/<r>/`, `src/transport/http/generated/<r>.rs` |
| `Types` | + above | `frontend/src/types/generated/<r>.ts`, `frontend/src/api/generated/<r>.ts` |
| `Composables` (default) | + above | `frontend/src/composables/generated/<r>.ts`, validators |
| `Components` | + above | `frontend/src/components/generated/forms/<r>/` |
| `Pages` | + above | `frontend/src/pages/generated/<r>/` (full CRUD UI) |

Pick the lowest level that ships your needed surface. `Composables` is right for "I'll write my own pages." `Pages` is right for "give me the admin shell now."

## Full example: `books` resource

```ron
ResourceState(
    schema_version: 2,
    name: "books",
    fields: {
        "id": FieldState(
            sql_type: "Int8",
            variants: [Db, Public, Admin],
            nullable: false,
            primary_key: true,
            validators: [],
        ),
        "title": FieldState(
            sql_type: "Text",
            variants: [Db, Insertable, Patch, Public, Admin],
            nullable: false,
            primary_key: false,
            validators: [Required, MaxLen(200)],
        ),
        "author_id": FieldState(
            sql_type: "Int8",
            variants: [Db, Insertable, Patch, Public, Admin],
            nullable: false,
            primary_key: false,
            validators: [Required],
        ),
        "published": FieldState(
            sql_type: "Bool",
            variants: [Db, Insertable, Patch, Public, Admin],
            nullable: false,
            primary_key: false,
            validators: [],
        ),
        "created_at": FieldState(
            sql_type: "Int8",
            variants: [Db, Public, Admin],
            nullable: false,
            primary_key: false,
            validators: [],
        ),
        "updated_at": FieldState(
            sql_type: "Int8",
            variants: [Db, Public, Admin],
            nullable: false,
            primary_key: false,
            validators: [],
        ),
        "deleted_at": FieldState(
            sql_type: "Int8",
            variants: [Db, Admin],
            nullable: true,
            primary_key: false,
            validators: [],
        ),
    },
    verbs: {
        List: VerbState(
            auth: Public,
            list_options: Some(ListOptions(
                paginated: true,
                filterable_columns: {
                    "title":     IlikeContains,
                    "author_id": Eq,
                    "published": Bool,
                },
                sortable_columns: ["created_at", "title"],
                default_sort: Some("created_at"),
                max_page_size: Some(100),
            )),
        ),
        Get:    VerbState(auth: Public,        list_options: None),
        Create: VerbState(auth: AuthRequired,  list_options: None),
        Update: VerbState(auth: AuthRequired,  list_options: None),
        Delete: VerbState(auth: AdminOnly,     list_options: None),
    },
    ws_events: Some(WsEventsState(
        trigger_columns: ["title", "published"],
        payload_shape: Public,
        topic_scope: PerRow,
    )),
    singular_override: None,
    soft_delete: Some(SoftDeleteConfig(
        column: "deleted_at",
        default_behavior: ExcludeDeleted,
    )),
    relations: {
        "author": BelongsTo(table: "users", fk_local_field: "author_id"),
    },
    gen_level: Pages,
)
```

The matching SQL migration:

```sql
-- migrations/<timestamp>_books/up.sql
CREATE TABLE books (
    id          BIGSERIAL PRIMARY KEY,
    title       TEXT      NOT NULL,
    author_id   BIGINT    NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    published   BOOLEAN   NOT NULL DEFAULT false,
    created_at  BIGINT    NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    updated_at  BIGINT    NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
    deleted_at  BIGINT
);
CREATE INDEX books_author_id ON books(author_id);
CREATE INDEX books_created_at ON books(created_at);
```

```sql
-- migrations/<timestamp>_books/down.sql
DROP TABLE books;
```

## Authoring workflow

```bash
# 1. Write SQL migration
mkdir migrations/$(date +%Y-%m-%d-%H%M%S)_books
$EDITOR migrations/*_books/up.sql      # CREATE TABLE
$EDITOR migrations/*_books/down.sql    # DROP TABLE

# 2. Apply schema
blast migrate
blast schema                           # refresh src/database/schema.rs

# 3. Author the Primer
$EDITOR storage/blast/state/resources/books.ron

# 4. Codegen + build
blast gen all
cargo build
```

Codegen will fail loudly if:
- A field's `sql_type` doesn't match `schema.rs` for that column
- A `Filter`/`Sort` references a column not in `fields`
- A `Relation` references a non-existent table
- The PK count is anything other than `1`

Errors point at the source line in the RON file.

## Hard rules

- **One PK per resource.** Composite PKs not supported in v2.
- **`schema_version` must be `2`.** Loader will refuse anything else (or upgrade if a v1→v2 upgrader is registered).
- **Field name = SQL column name.** Codegen does not rename.
- **Verbs are independent.** You may declare just `List` and `Get` (read-only resource).
- **`fields` order is preserved** in some emitter passes — order it however reads naturally.
- **Don't hand-edit `<layer>/generated/`** — those get rewritten on `blast gen`. Edit the Primer instead.

## Related specs

- `SPEC_ARCHITECTURE.md` — layer rules driving what each verb generates
- `SPEC_FLOWS.md` — flow body shape (auth + Crank + routine call)
- `SPEC_VALIDATORS.md` — validator codegen Rust + TS
- `SPEC_RELAY.md` — WebSocket events
- `SPEC_FRONTEND.md` — composables / pages / components consumption
- `blast/src/state/resource.rs` — authoritative Rust types
