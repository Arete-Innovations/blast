//! Auto-derived scope-method emission per FilterKind.
//!
//! Scopes are query-builder methods that push a Diesel filter clause onto
//! the boxed inner query. The exact method shape per kind is locked by
//! the design spec.

use crate::{
    codegen::models::filter_kind::{classify, fk_target, refine_for_column, FilterKind},
    state::{FieldName, FieldState},
};

/// Public emission entry point. Walks the resource fields once and emits
/// a flat block of impl-method bodies (without the surrounding impl
/// braces — `builder.rs` wraps them).
pub fn emit_for_field(out: &mut String, table: &str, field_name: &FieldName, field: &FieldState) {
    let col = field_name.as_str();
    let raw_kind = classify(field);
    let kind = refine_for_column(col, raw_kind);

    match kind.has_scopes() {
        false => return,
        true => {}
    }

    match kind {
        FilterKind::Bool => emit_bool(out, table, col),
        FilterKind::TimestampInt64 => emit_timestamp_int64(out, table, col),
        FilterKind::TimestampChrono => emit_timestamp_chrono(out, table, col),
        FilterKind::Int | FilterKind::Float | FilterKind::Decimal => {
            let rust_ty = scalar_rust_type(kind, field);
            emit_scalar_compare(out, table, col, &rust_ty);
            match fk_target(col, kind) {
                Some(target) => emit_fk_shortcut(out, table, col, target, &rust_ty),
                None => {}
            }
        }
        FilterKind::Text => emit_text(out, table, col),
        FilterKind::Uuid => emit_uuid(out, table, col),
        FilterKind::Enum => emit_enum(out, table, col, field),
        FilterKind::Skipped => return,
    }

    match field.nullable {
        true => emit_null_guards(out, table, col),
        false => {}
    }
}

fn emit_bool(out: &mut String, table: &str, col: &str) {
    let body = format!(
        r#"    /// Filter to rows where `{col}` is `true`.
    pub fn {col}(mut self) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.eq(true));
        self
    }}

    /// Filter to rows where `{col}` is `false`.
    pub fn not_{col}(mut self) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.eq(false));
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_timestamp_int64(out: &mut String, table: &str, col: &str) {
    let body = format!(
        r#"    /// Rows whose `{col}` epoch-second value is strictly before `t`.
    pub fn before_{col}(mut self, t: i64) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.lt(t));
        self
    }}

    /// Rows whose `{col}` is strictly after `t`.
    pub fn after_{col}(mut self, t: i64) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.gt(t));
        self
    }}

    /// Rows whose `{col}` falls within `[a, b]` inclusive.
    pub fn between_{col}(mut self, a: i64, b: i64) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.between(a, b));
        self
    }}

    /// Rows whose `{col}` falls within today (UTC, epoch seconds).
    pub fn {col}_today(self) -> Self {{
        let now = ::chrono::Utc::now();
        let day_start = match now.date_naive().and_hms_opt(0, 0, 0) {{
            Some(naive) => naive.and_utc().timestamp(),
            None => now.timestamp(),
        }};
        let end = day_start + 86_400;
        self.between_{col}(day_start, end)
    }}

    /// Rows whose `{col}` falls within the last 7 days (UTC, epoch seconds).
    pub fn {col}_this_week(self) -> Self {{
        let end = ::chrono::Utc::now().timestamp();
        let start = end - 7 * 86_400;
        self.between_{col}(start, end)
    }}

    /// Rows whose `{col}` falls within the last `n` days (UTC, epoch seconds).
    pub fn {col}_last_n_days(self, n: i64) -> Self {{
        let end = ::chrono::Utc::now().timestamp();
        let start = end - n * 86_400;
        self.between_{col}(start, end)
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_timestamp_chrono(out: &mut String, table: &str, col: &str) {
    let body = format!(
        r#"    /// Rows whose `{col}` is strictly before `t`.
    pub fn before_{col}(mut self, t: ::chrono::DateTime<::chrono::Utc>) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.lt(t));
        self
    }}

    /// Rows whose `{col}` is strictly after `t`.
    pub fn after_{col}(mut self, t: ::chrono::DateTime<::chrono::Utc>) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.gt(t));
        self
    }}

    /// Rows whose `{col}` falls within `[a, b]` inclusive.
    pub fn between_{col}(
        mut self,
        a: ::chrono::DateTime<::chrono::Utc>,
        b: ::chrono::DateTime<::chrono::Utc>,
    ) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.between(a, b));
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_scalar_compare(out: &mut String, table: &str, col: &str, rust_ty: &str) {
    let body = format!(
        r#"    /// `WHERE {col} = ?`
    pub fn where_{col}_eq(mut self, v: {rust_ty}) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.eq(v));
        self
    }}

    /// `WHERE {col} > ?`
    pub fn where_{col}_gt(mut self, v: {rust_ty}) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.gt(v));
        self
    }}

    /// `WHERE {col} < ?`
    pub fn where_{col}_lt(mut self, v: {rust_ty}) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.lt(v));
        self
    }}

    /// `WHERE {col} BETWEEN a AND b` inclusive.
    pub fn where_{col}_between(mut self, a: {rust_ty}, b: {rust_ty}) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.between(a, b));
        self
    }}

    /// `WHERE {col} IN (...)`. Pass an empty slice to match nothing.
    pub fn where_{col}_in(mut self, vs: &[{rust_ty}]) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        let owned: Vec<{rust_ty}> = vs.to_vec();
        self.inner = self.inner.filter(schema::{col}.eq_any(owned));
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_fk_shortcut(out: &mut String, table: &str, col: &str, target: &str, rust_ty: &str) {
    let body = format!(
        r#"    /// Filter to rows whose foreign key `{col}` references the given `{target}` id.
    pub fn by_{target}(mut self, id: {rust_ty}) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.eq(id));
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_text(out: &mut String, table: &str, col: &str) {
    let body = format!(
        r#"    /// `WHERE {col} = ?`
    pub fn where_{col}_eq(mut self, s: impl Into<String>) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.eq(s.into()));
        self
    }}

    /// Case-insensitive `ILIKE %?%`.
    pub fn where_{col}_contains(mut self, s: impl AsRef<str>) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        let pat = format!("%{{}}%", s.as_ref());
        self.inner = self.inner.filter(schema::{col}.ilike(pat));
        self
    }}

    /// Case-insensitive `ILIKE ?%`.
    pub fn where_{col}_starts_with(mut self, s: impl AsRef<str>) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        let pat = format!("{{}}%", s.as_ref());
        self.inner = self.inner.filter(schema::{col}.ilike(pat));
        self
    }}

    /// Case-insensitive `ILIKE %?`.
    pub fn where_{col}_ends_with(mut self, s: impl AsRef<str>) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        let pat = format!("%{{}}", s.as_ref());
        self.inner = self.inner.filter(schema::{col}.ilike(pat));
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_uuid(out: &mut String, table: &str, col: &str) {
    let body = format!(
        r#"    /// Filter to the row matching this `{col}` UUID.
    pub fn by_{col}(mut self, v: ::uuid::Uuid) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.eq(v));
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_enum(out: &mut String, table: &str, col: &str, field: &FieldState) {
    let raw = field.sql_type.as_str();
    let stem_lower = match raw.strip_suffix("_enum") {
        Some(stem) => stem,
        None => raw,
    };
    let stem_pascal = pascalize(stem_lower);

    let body = format!(
        r#"    /// Filter to rows whose `{col}` matches the given enum variant.
    pub fn where_{col}(mut self, v: crate::structs::generated::{table}::{stem_pascal}) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.eq(v));
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn pascalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut up = true;
    for ch in input.chars() {
        match ch {
            '_' | '-' => up = true,
            other => match up {
                true => {
                    for u in other.to_uppercase() {
                        out.push(u);
                    }
                    up = false;
                }
                false => out.push(other),
            },
        }
    }
    out
}

fn emit_null_guards(out: &mut String, table: &str, col: &str) {
    let body = format!(
        r#"    /// Filter to rows where `{col}` IS NULL.
    pub fn {col}_null(mut self) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.is_null());
        self
    }}

    /// Filter to rows where `{col}` IS NOT NULL.
    pub fn {col}_not_null(mut self) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.is_not_null());
        self
    }}
"#,
    );
    out.push_str(&body);
}

fn scalar_rust_type(kind: FilterKind, field: &FieldState) -> String {
    let sql = field.sql_type.as_str().to_ascii_lowercase();
    match (kind, sql.as_str()) {
        (FilterKind::Int, "int2") | (FilterKind::Int, "smallint") | (FilterKind::Int, "smallserial") => "i16".to_string(),
        (FilterKind::Int, "int4") | (FilterKind::Int, "integer") | (FilterKind::Int, "serial") => "i32".to_string(),
        (FilterKind::Int, "int8") | (FilterKind::Int, "bigint") | (FilterKind::Int, "bigserial") => "i64".to_string(),
        (FilterKind::Int, _other) => "i32".to_string(),
        (FilterKind::Float, "float4") | (FilterKind::Float, "real") => "f32".to_string(),
        (FilterKind::Float, _other) => "f64".to_string(),
        (FilterKind::Decimal, _) => "::rust_decimal::Decimal".to_string(),
        (FilterKind::Bool, _) | (FilterKind::TimestampInt64, _) | (FilterKind::TimestampChrono, _) | (FilterKind::Text, _) | (FilterKind::Uuid, _) | (FilterKind::Enum, _) | (FilterKind::Skipped, _) => "i64".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::state::{names::SqlType, FieldVariant};

    fn field(sql: &str, nullable: bool) -> FieldState {
        let mut variants = BTreeSet::new();
        variants.insert(FieldVariant::Db);
        FieldState {
            sql_type: SqlType::new(sql),
            variants,
            nullable,
            primary_key: false,
            validators: BTreeSet::new(),
        }
    }

    #[test]
    fn bool_emits_paired_scopes() {
        let mut out = String::new();
        emit_for_field(&mut out, "users", &FieldName::new("active"), &field("Bool", false));
        assert!(out.contains("pub fn active(mut self) -> Self"));
        assert!(out.contains("pub fn not_active(mut self) -> Self"));
        assert!(out.contains("schema::active.eq(true)"));
        assert!(out.contains("schema::active.eq(false)"));
    }

    #[test]
    fn timestamp_int64_emits_helpers_for_at_columns() {
        let mut out = String::new();
        emit_for_field(&mut out, "users", &FieldName::new("created_at"), &field("Int8", false));
        assert!(out.contains("pub fn before_created_at(mut self, t: i64) -> Self"));
        assert!(out.contains("pub fn after_created_at(mut self, t: i64) -> Self"));
        assert!(out.contains("pub fn between_created_at(mut self, a: i64, b: i64) -> Self"));
        assert!(out.contains("pub fn created_at_today(self) -> Self"));
        assert!(out.contains("pub fn created_at_this_week(self) -> Self"));
        assert!(out.contains("pub fn created_at_last_n_days(self, n: i64) -> Self"));
    }

    #[test]
    fn timestamp_chrono_emits_typed_helpers() {
        let mut out = String::new();
        emit_for_field(&mut out, "events", &FieldName::new("happened_at"), &field("Timestamptz", false));
        assert!(out.contains("::chrono::DateTime<::chrono::Utc>"));
        assert!(out.contains("pub fn before_happened_at"));
    }

    #[test]
    fn int_emits_compare_in_and_optional_fk() {
        let mut out = String::new();
        emit_for_field(&mut out, "posts", &FieldName::new("author_id"), &field("Int8", false));
        assert!(out.contains("pub fn where_author_id_eq"));
        assert!(out.contains("pub fn where_author_id_gt"));
        assert!(out.contains("pub fn where_author_id_lt"));
        assert!(out.contains("pub fn where_author_id_between"));
        assert!(out.contains("pub fn where_author_id_in"));
        assert!(out.contains("pub fn by_author"), "FK shortcut should be emitted for *_id columns");
    }

    #[test]
    fn text_emits_ilike_variants() {
        let mut out = String::new();
        emit_for_field(&mut out, "users", &FieldName::new("email"), &field("Varchar", false));
        assert!(out.contains("pub fn where_email_eq"));
        assert!(out.contains("pub fn where_email_contains"));
        assert!(out.contains("pub fn where_email_starts_with"));
        assert!(out.contains("pub fn where_email_ends_with"));
        assert!(out.contains(".ilike("));
    }

    #[test]
    fn uuid_emits_by_method() {
        let mut out = String::new();
        emit_for_field(&mut out, "tokens", &FieldName::new("public_id"), &field("Uuid", false));
        assert!(out.contains("pub fn by_public_id"));
        assert!(out.contains("::uuid::Uuid"));
    }

    #[test]
    fn enum_emits_single_method_no_variant_sugar() {
        let mut out = String::new();
        emit_for_field(&mut out, "tickets", &FieldName::new("status"), &field("status_enum", false));
        assert!(out.contains("pub fn where_status"));
        assert!(!out.contains("pub fn is_open"));
    }

    #[test]
    fn nullable_field_emits_null_guards() {
        let mut out = String::new();
        emit_for_field(&mut out, "users", &FieldName::new("nickname"), &field("Varchar", true));
        assert!(out.contains("pub fn nickname_null"));
        assert!(out.contains("pub fn nickname_not_null"));
    }

    #[test]
    fn jsonb_emits_no_scopes() {
        let mut out = String::new();
        emit_for_field(&mut out, "events", &FieldName::new("payload"), &field("Jsonb", false));
        assert!(out.is_empty());
    }
}
