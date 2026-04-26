use crate::state::names::{AuthScopeField, FieldName, ResourceName, SqlType};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const RESOURCE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceState {
    pub schema_version: u32,
    pub name: ResourceName,
    pub fields: IndexMap<FieldName, FieldState>,
    pub verbs: IndexMap<Verb, VerbState>,
    #[serde(default)]
    pub ws_events: Option<WsEventsState>,
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
    MinValue(i64),
    MaxValue(i64),
    Pattern(String),
    OneOf(Vec<String>),
    Email,
    Url,
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
}
