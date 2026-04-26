//! Aggregation method emission — count, exists, first.
//!
//! Two emission targets: an auto-conn `count()` on the type itself for
//! the global row count, and `count` / `exists` / `first` on the in-flight
//! query builder. Module fns under the builder consume `self` because the
//! inner Diesel boxed query is not Clone once filters have been pushed.

/// Emit the auto-conn `count()` method on `impl <Type>`.
pub fn emit_type_count(out: &mut String, table: &str, stem: &str) {
    let body = format!(
        r#"    /// Total number of rows in `{table}`. Auto-acquires a connection.
    pub async fn count() -> ::std::result::Result<i64, ::catalyst::meltdown::MeltDown> {{
        use ::diesel_async::RunQueryDsl;
        let mut conn = ::catalyst::database::pool().get().await?;
        let n: i64 = ::diesel::QueryDsl::count(crate::database::schema::{table}::dsl::{table})
            .get_result(&mut conn)
            .await
            .map_err(|e: ::diesel::result::Error| match e {{
                ::diesel::result::Error::NotFound => {{
                    ::catalyst::meltdown::MeltDown::not_found("{table}", "count".to_string())
                }}
                other => ::catalyst::meltdown::MeltDown::from(other),
            }})?;
        Ok(n)
    }}

    /// Convenience: returns the typed builder for chaining filters.
    pub fn query() -> {stem}Query {{
        {stem}Query::new()
    }}
"#,
    );
    out.push_str(&body);
}

/// Emit the three terminal aggregations on `impl <Type>Query`.
///
/// `_table` is reserved for upcoming dsl-aware error context that
/// references the resource name in the failure path.
pub fn emit_query_aggregations(out: &mut String, _table: &str, stem: &str) {
    let body = format!(
        r#"    /// Count rows matching the in-flight filters.
    pub async fn count(self) -> ::std::result::Result<i64, ::catalyst::meltdown::MeltDown> {{
        use ::diesel_async::RunQueryDsl;
        let mut conn = ::catalyst::database::pool().get().await?;
        let n: i64 = ::diesel::QueryDsl::count(self.inner)
            .get_result(&mut conn)
            .await?;
        Ok(n)
    }}

    /// `true` when at least one row matches the in-flight filters.
    pub async fn exists(self) -> ::std::result::Result<bool, ::catalyst::meltdown::MeltDown> {{
        let n = self.count().await?;
        Ok(n > 0)
    }}

    /// Return the first matching row, if any.
    pub async fn first(
        self,
    ) -> ::std::result::Result<::std::option::Option<{stem}>, ::catalyst::meltdown::MeltDown> {{
        use ::diesel_async::RunQueryDsl;
        let mut conn = ::catalyst::database::pool().get().await?;
        let rows: ::std::vec::Vec<{stem}> = ::diesel::QueryDsl::limit(self.inner, 1)
            .load::<{stem}>(&mut conn)
            .await?;
        Ok(rows.into_iter().next())
    }}
"#,
    );
    out.push_str(&body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_count_emits_pool_call() {
        let mut out = String::new();
        emit_type_count(&mut out, "users", "User");
        assert!(out.contains("pub async fn count()"));
        assert!(out.contains("::catalyst::database::pool()"));
        assert!(out.contains("MeltDown::not_found(\"users\""));
        assert!(
            out.contains("pub fn query() -> UserQuery"),
            "should expose builder shortcut"
        );
    }

    #[test]
    fn query_aggregations_emit_count_exists_first() {
        let mut out = String::new();
        emit_query_aggregations(&mut out, "users", "User");
        assert!(out.contains("pub async fn count(self)"));
        assert!(out.contains("pub async fn exists(self)"));
        assert!(out.contains("pub async fn first(\n        self,"));
        assert!(out.contains("Option<User>"));
    }

    #[test]
    fn aggregations_use_pool_for_auto_conn() {
        let mut out = String::new();
        emit_query_aggregations(&mut out, "posts", "Post");
        assert!(out.contains("::catalyst::database::pool()"));
    }
}
