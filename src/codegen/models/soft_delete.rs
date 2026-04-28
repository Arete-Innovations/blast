//! Soft-delete scope and mutation emission.
//!
//! When a resource declares soft-delete, this module emits the three
//! query-builder scopes for visibility (with-deleted, exclude-deleted,
//! only-deleted), the constructor-level default filter, and a delete fn
//! body that does an UPDATE-stamp instead of a hard DELETE.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftDeleteConfig {
    /// Column carrying the deletion timestamp (epoch seconds in this stack).
    pub column: String,
    pub default_behavior: DefaultBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultBehavior {
    /// Constructor returns rows whose column IS NULL by default.
    Exclude,
    /// Constructor returns ALL rows by default.
    Include,
    /// Constructor returns rows whose column IS NOT NULL by default.
    OnlyDeleted,
}

/// Emit the three scope methods inside the query builder impl.
pub fn emit_scope_methods(out: &mut String, table: &str, cfg: &SoftDeleteConfig) {
    let col = &cfg.column;
    let body = format!(
        r#"    /// Lift the soft-delete filter — return both live and deleted rows.
    pub fn with_deleted(mut self) -> Self {{
        self.inner = crate::database::schema::{table}::dsl::{table}.into_boxed();
        self
    }}

    /// Apply the live-rows-only filter (`{col}` IS NULL).
    pub fn exclude_deleted(mut self) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self.inner.filter(schema::{col}.is_null());
        self
    }}

    /// Restrict to soft-deleted rows only (`{col}` IS NOT NULL).
    pub fn only_deleted(mut self) -> Self {{
        use crate::database::schema::{table}::dsl as schema;
        self.inner = self
            .inner
            .filter(schema::{col}.is_not_null());
        self
    }}
"#,
    );
    out.push_str(&body);
}

/// Emit the line that pre-applies the default behavior inside the
/// constructor body. Suitable to be concatenated after the boxed-query
/// initializer.
pub fn emit_default_application(out: &mut String, table: &str, cfg: &SoftDeleteConfig) {
    let col = &cfg.column;
    match cfg.default_behavior {
        DefaultBehavior::Include => {
            out.push_str("        // soft-delete: default = include all rows\n");
        }
        DefaultBehavior::Exclude => {
            let line = format!("        let inner = inner.filter(crate::database::schema::{table}::dsl::{col}.is_null());\n");
            out.push_str(&line);
        }
        DefaultBehavior::OnlyDeleted => {
            let line = format!("        let inner = inner.filter(crate::database::schema::{table}::dsl::{col}.is_not_null());\n");
            out.push_str(&line);
        }
    }
}

/// Emit the soft-delete `delete` body that replaces the hard DELETE.
/// Caller wraps this inside a delete fn returning `Result<(), MeltDown>`.
pub fn emit_delete_body(out: &mut String, table: &str, cfg: &SoftDeleteConfig) {
    let col = &cfg.column;
    let body = format!(
        r#"    let now = ::chrono::Utc::now().timestamp();
    use crate::database::schema::{table}::dsl as schema;
    let n = ::diesel_async::RunQueryDsl::execute(
        ::diesel::update(schema::{table}.filter(schema::id.eq(id))).set(schema::{col}.eq(now)),
        conn,
    )
    .await?;
    if n == 0 {{
        return Err(::catalyst::meltdown::MeltDown::not_found("{table}", id.to_string()));
    }}
    Ok(())
"#,
    );
    out.push_str(&body);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> SoftDeleteConfig {
        SoftDeleteConfig {
            column: "deleted_at".to_string(),
            default_behavior: DefaultBehavior::Exclude,
        }
    }

    #[test]
    fn emits_three_scope_methods() {
        let mut out = String::new();
        emit_scope_methods(&mut out, "users", &cfg());
        assert!(out.contains("pub fn with_deleted(mut self) -> Self"));
        assert!(out.contains("pub fn exclude_deleted(mut self) -> Self"));
        assert!(out.contains("pub fn only_deleted(mut self) -> Self"));
        assert!(out.contains("schema::deleted_at.is_null()"));
        assert!(out.contains("schema::deleted_at.is_not_null()"));
    }

    #[test]
    fn default_exclude_applies_is_null_filter() {
        let mut out = String::new();
        emit_default_application(&mut out, "users", &cfg());
        assert!(out.contains("filter("));
        assert!(out.contains("deleted_at"));
        assert!(out.contains("is_null()"));
    }

    #[test]
    fn default_include_emits_no_filter() {
        let mut out = String::new();
        emit_default_application(
            &mut out,
            "users",
            &SoftDeleteConfig {
                column: "deleted_at".to_string(),
                default_behavior: DefaultBehavior::Include,
            },
        );
        assert!(out.contains("// soft-delete: default = include"));
        assert!(!out.contains("is_null"));
    }

    #[test]
    fn default_only_deleted_applies_not_null_filter() {
        let mut out = String::new();
        emit_default_application(
            &mut out,
            "users",
            &SoftDeleteConfig {
                column: "deleted_at".to_string(),
                default_behavior: DefaultBehavior::OnlyDeleted,
            },
        );
        assert!(out.contains("is_not_null"));
    }

    #[test]
    fn delete_body_uses_update_not_delete() {
        let mut out = String::new();
        emit_delete_body(&mut out, "users", &cfg());
        assert!(out.contains("::diesel::update("), "soft-delete must use UPDATE, not DELETE");
        assert!(out.contains(".set(schema::deleted_at.eq(now))"));
        assert!(!out.contains("::diesel::delete"));
        assert!(out.contains("MeltDown::not_found"));
    }
}
