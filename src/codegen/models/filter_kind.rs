//! Local classification for SQL types into the buckets that drive
//! auto-derived scope emission.
//!
//! The `state-extensions` parallel branch will introduce a richer
//! per-field FilterKind at `crate::state`. Until that lands we synthesize
//! the bucket from `FieldState.sql_type` plus the field's existing
//! nullability marker — same coverage, no policy knobs.

use crate::state::FieldState;

/// Classification used by `scopes::emit_for_field`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Bool,
    /// `i64` epoch second/ms timestamp. Maps to date-window scope helpers.
    TimestampInt64,
    /// `chrono::DateTime<Utc>` / `chrono::NaiveDateTime`.
    TimestampChrono,
    Int,
    /// Opaque integer identifier — primary key or `*_id` foreign key.
    /// Only `eq`/`in` and the `by_<target>` shortcut make sense; ordering
    /// (`gt`/`lt`/`between`) is meaningless on a surrogate key.
    OpaqueId,
    Float,
    Decimal,
    Text,
    Uuid,
    /// Native enum referenced by name.
    Enum,
    /// `JSONB`/`JSON`/Array — explicitly skipped per locked design.
    Skipped,
}

impl FilterKind {
    /// `true` if this kind contributes any auto-scope methods.
    pub fn has_scopes(self) -> bool {
        match self {
            FilterKind::Skipped => false,
            FilterKind::Bool | FilterKind::TimestampInt64 | FilterKind::TimestampChrono | FilterKind::Int | FilterKind::OpaqueId | FilterKind::Float | FilterKind::Decimal | FilterKind::Text | FilterKind::Uuid | FilterKind::Enum => true,
        }
    }
}

/// Map the raw SQL type string to a `FilterKind` bucket. Comparison is
/// ASCII-case-insensitive: state files written by hand may carry
/// uppercase identifiers while the wizard emits Pascal case. Both should
/// resolve identically.
pub fn classify(field: &FieldState) -> FilterKind {
    let sql = field.sql_type.as_str().to_ascii_lowercase();
    match scalar_kind(&sql) {
        Some(kind) => kind,
        None => match sql.as_str() {
            "enum" => FilterKind::Enum,
            other => match other.ends_with("_enum") {
                true => FilterKind::Enum,
                false => FilterKind::Text,
            },
        },
    }
}

fn scalar_kind(sql: &str) -> Option<FilterKind> {
    match sql {
        "bool" | "boolean" => Some(FilterKind::Bool),
        "int2" | "smallint" | "smallserial" | "int4" | "integer" | "serial" => Some(FilterKind::Int),
        "int8" | "bigint" | "bigserial" => Some(FilterKind::Int),
        "float4" | "real" | "float8" | "double" | "double precision" => Some(FilterKind::Float),
        "numeric" | "decimal" => Some(FilterKind::Decimal),
        "text" | "varchar" | "bpchar" | "char" | "citext" => Some(FilterKind::Text),
        "uuid" => Some(FilterKind::Uuid),
        "timestamp" | "timestamptz" => Some(FilterKind::TimestampChrono),
        "json" | "jsonb" => Some(FilterKind::Skipped),
        unrecognized => map_unrecognized_sql(unrecognized),
    }
}

/// Catch-all bucket for SQL types Blast hasn't taught itself about yet.
/// Returns `None` so `classify` can fall through to the enum / text
/// fallback.
fn map_unrecognized_sql(_sql: &str) -> Option<FilterKind> {
    None
}

/// Column-name heuristic: when the column is named `created_at`,
/// `updated_at`, or `*_timestamp` AND the underlying SQL type is an
/// integer, upgrade the bucket to `TimestampInt64` so the timestamp scope
/// helpers fire. When the column is an opaque identifier (the PK column
/// `id` or any `*_id` foreign key) downgrade to `OpaqueId` — ordering
/// helpers (`gt`/`lt`/`between`) are meaningless on surrogate keys.
pub fn refine_for_column(name: &str, kind: FilterKind) -> FilterKind {
    match kind {
        FilterKind::Int => match looks_like_timestamp_column(name) {
            true => FilterKind::TimestampInt64,
            false => match looks_like_opaque_id(name) {
                true => FilterKind::OpaqueId,
                false => FilterKind::Int,
            },
        },
        FilterKind::Bool | FilterKind::TimestampInt64 | FilterKind::TimestampChrono | FilterKind::OpaqueId | FilterKind::Float | FilterKind::Decimal | FilterKind::Text | FilterKind::Uuid | FilterKind::Enum | FilterKind::Skipped => kind,
    }
}

fn looks_like_opaque_id(name: &str) -> bool {
    name == "id" || name.ends_with("_id")
}

fn looks_like_timestamp_column(name: &str) -> bool {
    name.ends_with("_at") || name.ends_with("_timestamp")
}

/// Returns the FK shortcut name when the column is `<related>_id` AND we
/// classified it as `OpaqueId`. Returns `None` for non-FK columns.
pub fn fk_target(name: &str, kind: FilterKind) -> Option<&str> {
    match matches!(kind, FilterKind::OpaqueId) {
        false => None,
        true => match name.strip_suffix("_id") {
            Some(stem) => fk_stem_filter(stem),
            None => None, // allow: column without `_id` suffix is not an FK; signaling absence is the contract
        },
    }
}

/// Reject empty stems (the `_id` column itself); pass through otherwise.
fn fk_stem_filter(stem: &str) -> Option<&str> {
    match stem.is_empty() {
        true => None,
        false => Some(stem),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::state::{names::SqlType, FieldVariant};

    fn field(sql: &str) -> FieldState {
        let mut variants = BTreeSet::new();
        variants.insert(FieldVariant::Db);
        FieldState {
            sql_type: SqlType::new(sql),
            variants,
            nullable: false,
            primary_key: false,
            validators: BTreeSet::new(),
            kind: Default::default(),
        }
    }

    #[test]
    fn classifies_each_scalar_bucket() {
        assert_eq!(classify(&field("Bool")), FilterKind::Bool);
        assert_eq!(classify(&field("Int4")), FilterKind::Int);
        assert_eq!(classify(&field("Int8")), FilterKind::Int);
        assert_eq!(classify(&field("Float8")), FilterKind::Float);
        assert_eq!(classify(&field("Numeric")), FilterKind::Decimal);
        assert_eq!(classify(&field("Varchar")), FilterKind::Text);
        assert_eq!(classify(&field("Text")), FilterKind::Text);
        assert_eq!(classify(&field("Uuid")), FilterKind::Uuid);
        assert_eq!(classify(&field("Timestamptz")), FilterKind::TimestampChrono);
        assert_eq!(classify(&field("Jsonb")), FilterKind::Skipped);
    }

    #[test]
    fn enum_case_insensitive() {
        assert_eq!(classify(&field("status_enum")), FilterKind::Enum);
        assert_eq!(classify(&field("STATUS_ENUM")), FilterKind::Enum);
    }

    #[test]
    fn refine_upgrades_int8_at_columns() {
        assert_eq!(refine_for_column("created_at", FilterKind::Int), FilterKind::TimestampInt64);
        assert_eq!(refine_for_column("updated_at", FilterKind::Int), FilterKind::TimestampInt64);
        assert_eq!(refine_for_column("login_timestamp", FilterKind::Int), FilterKind::TimestampInt64);
        assert_eq!(refine_for_column("count", FilterKind::Int), FilterKind::Int);
    }

    #[test]
    fn refine_downgrades_id_columns_to_opaque_id() {
        assert_eq!(refine_for_column("id", FilterKind::Int), FilterKind::OpaqueId);
        assert_eq!(refine_for_column("user_id", FilterKind::Int), FilterKind::OpaqueId);
        assert_eq!(refine_for_column("author_id", FilterKind::Int), FilterKind::OpaqueId);
    }

    #[test]
    fn refine_does_not_change_non_int_kinds() {
        assert_eq!(refine_for_column("created_at", FilterKind::Text), FilterKind::Text);
        assert_eq!(refine_for_column("user_id", FilterKind::Text), FilterKind::Text);
    }

    #[test]
    fn fk_target_strips_id_suffix() {
        assert_eq!(fk_target("author_id", FilterKind::OpaqueId), Some("author"));
        assert_eq!(fk_target("user_id", FilterKind::OpaqueId), Some("user"));
        assert_eq!(fk_target("id", FilterKind::OpaqueId), None);
        assert_eq!(fk_target("name", FilterKind::OpaqueId), None);
        assert_eq!(fk_target("author_id", FilterKind::Int), None);
        assert_eq!(fk_target("author_id", FilterKind::Text), None);
    }

    #[test]
    fn skipped_has_no_scopes() {
        assert!(!FilterKind::Skipped.has_scopes());
        assert!(FilterKind::Bool.has_scopes());
    }
}
