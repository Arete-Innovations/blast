use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    error::BlastResult,
    state::{
        gen_level::GenLevel,
        names::{AuthScopeField, FieldName, ResourceName, SqlType},
    },
};

pub const RESOURCE_SCHEMA_VERSION: u32 = 3;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceState {
    pub schema_version: u32,
    pub name: ResourceName,
    pub fields: IndexMap<FieldName, FieldState>,
    pub verbs: IndexMap<Verb, VerbState>,
    #[serde(default)]
    pub ws_events: Option<WsEventsState>,
    /// Optional override for the singular form of `name` used by struct
    /// codegen (e.g. `data` → `Datum`). When `None`, the inflector picks
    /// the default singularization.
    #[serde(default)]
    pub singular_override: Option<String>,
    /// Soft-delete policy for this resource. When `Some`, the codegen
    /// emits delete-marker logic against the named column and respects
    /// the configured default visibility behavior in list/get queries.
    #[serde(default)]
    pub soft_delete: Option<SoftDeleteConfig>,
    /// Named relations to other tables, consumed by codegen to emit
    /// loaders/joins. Keyed by relation name (e.g. `"author"`).
    /// Many-to-many is intentionally not modeled in v2.
    #[serde(default)]
    pub relations: BTreeMap<String, Relation>,
    /// Per-resource codegen cut-off. Each level implies all prior levels.
    /// Default: `Composables`. Power-users opt up to `Pages` for admin-grade
    /// CRUD UI or down to `Struct` for data-only.
    #[serde(default)]
    pub gen_level: GenLevel,
    /// When set, the generated list page replaces the default
    /// `<TableBuilder>` with a vertical column of cells. The codegen emits
    /// `<{cell} item=row/>` per `Public` row inside the standard
    /// `AppShell`/`PageShell` chrome. The cell module path is
    /// `crate::views::components::vendored::{module}::{component}`.
    #[serde(default)]
    pub list_layout: Option<CustomLayout>,
    /// When set, the generated detail page replaces the default
    /// `<DetailBuilder>` with a single rendering of the cell.
    /// The codegen emits `<{cell} item=public/>`.
    #[serde(default)]
    pub detail_layout: Option<CustomLayout>,
    /// Atomic delete-or-create endpoint. When set, codegen emits a full
    /// toggle stack: `POST /api/<resource>/toggle/{<scope_field>}` →
    /// matches on `<scope_field>` plus all `FromSession` fields, deletes
    /// the row if present (returns `active: false`), inserts otherwise
    /// (returns `active: true`). Response carries the post-toggle row count
    /// scoped by `<scope_field>`.
    #[serde(default)]
    pub toggle_endpoint: Option<ToggleEndpoint>,

    /// Extra WS topics the detail page subscribes to in addition to the
    /// implicit `<table>:row:<id>`. Each entry may use the literal `{id}`
    /// placeholder, replaced at codegen time with the detail page's
    /// `id_signal.get()`. Useful for cross-resource live aggregates
    /// (e.g. tweets.live_topics = ["likes:tweet_id:{id}"] so a like-toggle
    /// refreshes the tweet detail page).
    #[serde(default)]
    pub live_topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleEndpoint {
    /// URL path-param column. The toggled row's match-tuple is
    /// `(scope_field=<path>, <FromSession field>=<session value>)`.
    pub scope_field: FieldName,
}

/// Replaces the default builder in a generated list/detail page with a
/// custom Leptos component. The component must accept a single prop named
/// `item` (or `items` for list) of the resource's `Public` type
/// (`Vec<Public>` for list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomLayout {
    /// Module path under `crate::views::components::vendored::`.
    /// Example: `"tweet_card"`.
    pub module: String,
    /// Component name within that module.
    /// Example: `"TweetCard"`.
    pub component: String,
}

/// A typed relation between this resource and another table.
///
/// `BelongsTo`: this resource carries the FK in `fk_local_field`,
/// pointing at `table.id`.
/// `HasMany`: the other `table` carries the FK in `fk_remote_field`,
/// pointing back at this resource's id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relation {
    BelongsTo { table: String, fk_local_field: FieldName },
    HasMany { table: String, fk_remote_field: FieldName },
}

/// Soft-delete policy attached to a resource.
///
/// When present, generated `delete` flows update `column` (typically
/// `deleted_at: Timestamptz`) instead of issuing a hard `DELETE`.
/// Generated read paths consult `default_behavior` to decide whether to
/// hide soft-deleted rows by default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftDeleteConfig {
    pub column: FieldName,
    pub default_behavior: SoftDeleteDefault,
}

/// Whether generated read paths return soft-deleted rows by default.
///
/// `ExcludeDeleted`: list/get filter `deleted_at IS NULL` unless the
/// caller explicitly opts in.
/// `IncludeDeleted`: list/get return all rows; callers must opt out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SoftDeleteDefault {
    ExcludeDeleted,
    IncludeDeleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldState {
    pub sql_type: SqlType,
    pub variants: BTreeSet<FieldVariant>,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default)]
    pub primary_key: bool,
    #[serde(default)]
    pub validators: BTreeSet<ValidatorRule>,
    /// UI/wire-shape hint. Drives form input control + create-handler injection.
    /// Default = `Default` (renders as appropriate `<input>`).
    /// `Textarea` renders multi-line input.
    /// `Hidden` omits the field from the form entirely (client must not send it).
    /// `FromSession(...)` omits from the form AND injects the session value
    /// into the insertable server-side before validation.
    #[serde(default)]
    pub kind: FieldKind,
}

/// Per-field UI / wire-shape directive consumed by form codegen and by the
/// generated HTTP create handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FieldKind {
    #[default]
    Default,
    Textarea,
    Hidden,
    FromSession(SessionFieldRef),
}

/// Which value off the request session should be injected when a field is
/// marked `FromSession`. Codegen rewrites this to a `ctx.session()` accessor
/// in the create handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SessionFieldRef {
    UserId,
    SessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FieldVariant {
    Db,
    Insertable,
    Patch,
    Public,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ValidatorRule {
    Required,
    MinLen(usize),
    MaxLen(usize),
    ExactLen(usize),
    MinValue(i64),
    MaxValue(i64),
    Positive,
    NonNegative,
    Negative,
    MultipleOf(i64),
    Pattern(String),
    OneOf(Vec<String>),
    Email,
    Url,
    Uuid,
    Slug,
    Alpha,
    AlphaNumeric,
    Numeric,
    Integer,
    Decimal,
    Ipv4,
    Ipv6,
    IpAddr,
    Hostname,
    HexColor,
    PhoneE164,
    Trimmed,
    NoWhitespace,
    Lowercase,
    Uppercase,
    Ascii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Verb {
    List,
    Get,
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerbState {
    pub auth: AuthMode,
    #[serde(default)]
    pub list_options: Option<ListOptions>,
    #[serde(default = "default_true")]
    pub emit_rest_api: bool,
    #[serde(default = "default_true")]
    pub emit_html_page: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    Public,
    AuthRequired,
    AdminOnly,
    ScopedTo(AuthScopeField),
    Roles(BTreeSet<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListOptions {
    pub paginated: bool,
    /// Map of column name → filter operator. The codegen emits a typed
    /// `<Type>Filter` struct and matching SQL predicates per `FilterKind`.
    pub filterable_columns: BTreeMap<FieldName, FilterKind>,
    #[serde(default)]
    pub sortable_columns: BTreeSet<FieldName>,
    #[serde(default)]
    pub default_sort: Option<FieldName>,
    #[serde(default)]
    pub max_page_size: Option<u32>,
}

/// How a filterable column is matched in generated SQL/TS code.
///
/// - `Eq`: exact match (`col = $1`)
/// - `Range`: inclusive range with `from`/`to` ends
/// - `IlikeContains`: case-insensitive substring (`col ILIKE '%$1%'`)
/// - `In`: any-of (`col = ANY($1)`)
/// - `Bool`: boolean toggle
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FilterKind {
    Eq,
    Range,
    IlikeContains,
    In,
    Bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsEventsState {
    pub trigger_columns: BTreeSet<FieldName>,
    pub payload_shape: PayloadShape,
    pub topic_scope: TopicScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayloadShape {
    Public,
    Admin,
    IdOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopicScope {
    Global,
    PerRow,
    ScopedTo(AuthScopeField),
}

impl ResourceState {
    pub fn new(name: ResourceName) -> Self {
        Self {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name,
            fields: IndexMap::new(),
            verbs: IndexMap::new(),
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: GenLevel::default(),
            list_layout: None,
            detail_layout: None,
            toggle_endpoint: None,
            live_topics: Vec::new(),
        }
    }

    pub fn canonicalize(&mut self) {
        let mut field_pairs: Vec<(FieldName, FieldState)> = self.fields.drain(..).collect();
        field_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in field_pairs {
            self.fields.insert(k, v);
        }

        let mut verb_pairs: Vec<(Verb, VerbState)> = self.verbs.drain(..).collect();
        verb_pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in verb_pairs {
            self.verbs.insert(k, v);
        }
    }

    /// Walks every name in the resource (resource name, field names, FK refs,
    /// soft-delete column, list-options filterable/sortable/default_sort,
    /// AuthMode::ScopedTo, ws trigger columns, TopicScope::ScopedTo) and
    /// validates each with the snake_case + Rust-keyword check. Run by
    /// `state::load_resource` after RON parse + upgrader so hand-edited files
    /// fail loud at load time instead of producing unparseable Rust later.
    pub fn validate_names(&self) -> BlastResult<()> {
        self.name.validate()?;
        for fname in self.fields.keys() {
            fname.validate()?;
        }
        for relation in self.relations.values() {
            match relation {
                Relation::BelongsTo { fk_local_field, .. } => fk_local_field.validate()?,
                Relation::HasMany { fk_remote_field, .. } => fk_remote_field.validate()?,
            }
        }
        match &self.soft_delete {
            Some(sd) => sd.column.validate()?,
            None => {}
        }
        for verb_state in self.verbs.values() {
            match &verb_state.auth {
                AuthMode::ScopedTo(field) => field.validate()?,
                AuthMode::Public | AuthMode::AuthRequired | AuthMode::AdminOnly | AuthMode::Roles(_) => {}
            }
            match &verb_state.list_options {
                Some(opts) => {
                    for fname in opts.filterable_columns.keys() {
                        fname.validate()?;
                    }
                    for fname in &opts.sortable_columns {
                        fname.validate()?;
                    }
                    match &opts.default_sort {
                        Some(default) => default.validate()?,
                        None => {}
                    }
                }
                None => {}
            }
        }
        match &self.ws_events {
            Some(ws) => {
                for fname in &ws.trigger_columns {
                    fname.validate()?;
                }
                match &ws.topic_scope {
                    TopicScope::ScopedTo(field) => field.validate()?,
                    TopicScope::Global | TopicScope::PerRow => {}
                }
            }
            None => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_state_emit_flags_default_true_via_serde() {
        let raw = r#"(
            auth: Public,
            list_options: None,
        )"#;
        let parsed: VerbState = match ron::from_str(raw) {
            Ok(v) => v,
            Err(e) => panic!("parse: {e}"),
        };
        assert!(parsed.emit_rest_api, "emit_rest_api defaults to true");
        assert!(parsed.emit_html_page, "emit_html_page defaults to true");
    }

    #[test]
    fn verb_state_emit_flags_round_trip_when_set_false() {
        let original = VerbState {
            auth: AuthMode::Public,
            list_options: None,
            emit_rest_api: false,
            emit_html_page: false,
        };
        let body = match ron::to_string(&original) {
            Ok(s) => s,
            Err(e) => panic!("serialize: {e}"),
        };
        let parsed: VerbState = match ron::from_str(&body) {
            Ok(v) => v,
            Err(e) => panic!("parse: {e} body={body}"),
        };
        assert_eq!(parsed, original, "round-trip preserves both flags");
        assert!(!parsed.emit_rest_api);
        assert!(!parsed.emit_html_page);
    }
}
