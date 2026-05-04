//! Type-method auto-conn wrappers — the clean callsite used 95% of the time.
//!
//! Emits a per-type impl block whose methods auto-acquire a pooled
//! connection and forward to the txn-flavoured module-level fns.
//! Auto-acquire is the right default for transport callers; flows wanting
//! txn-bound semantics call the module fns directly with their own
//! borrowed connection.
//!
//! NOTE: the pool accessor is supplied by the parallel bootstrap-pool
//! agent. Until that lands, generated user-app code will fail to compile
//! against the missing item — by design.

use crate::codegen::models::{module_fns::VerbSelection, naming};

pub fn emit_impl_block(out: &mut String, table: &str, stem: &str, verbs: &VerbSelection) {
    let insertable = naming::insertable_type(stem);
    let patch = naming::patch_type(stem);

    let header = format!("impl {stem} {{\n");
    out.push_str(&header);
    if verbs.list {
        emit_list_wrapper(out, stem);
        out.push('\n');
    }
    if verbs.get {
        emit_get_wrapper(out, stem);
        out.push('\n');
    }
    if verbs.create {
        emit_create_wrapper(out, stem, &insertable);
        out.push('\n');
    }
    if verbs.update {
        emit_update_wrapper(out, stem, &patch);
        out.push('\n');
    }
    if verbs.delete {
        emit_delete_wrapper(out);
        out.push('\n');
    }
    if verbs.list {
        emit_list_from_query_wrapper(out, table, stem);
    }
    out.push_str("}\n");
}

fn emit_list_wrapper(out: &mut String, stem: &str) {
    let body = format!(
        r#"    /// Page-fetch with auto-acquired connection. Convenience wrapper around
    /// the txn-friendly module-level list fn.
    pub async fn list(
        query: &crate::structs::list_query::ListQuery,
    ) -> ::std::result::Result<
        crate::structs::list_query::ListResponse<{stem}>,
        crate::meltdown::MeltDown,
    > {{
        let mut conn = crate::database::acquire_conn().await?;
        self::list(&mut conn, query).await
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_get_wrapper(out: &mut String, stem: &str) {
    let body = format!(
        r#"    /// Auto-conn variant of the module-level get fn.
    pub async fn get(id: i64) -> ::std::result::Result<{stem}, crate::meltdown::MeltDown> {{
        let mut conn = crate::database::acquire_conn().await?;
        self::get(&mut conn, id).await
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_create_wrapper(out: &mut String, stem: &str, insertable: &str) {
    let body = format!(
        r#"    /// Auto-conn variant of the module-level create fn.
    pub async fn create(
        input: &{insertable},
    ) -> ::std::result::Result<{stem}, crate::meltdown::MeltDown> {{
        let mut conn = crate::database::acquire_conn().await?;
        self::create(&mut conn, input).await
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_update_wrapper(out: &mut String, stem: &str, patch: &str) {
    let body = format!(
        r#"    /// Auto-conn variant of the module-level update fn.
    pub async fn update(
        id: i64,
        patch: &{patch},
    ) -> ::std::result::Result<{stem}, crate::meltdown::MeltDown> {{
        let mut conn = crate::database::acquire_conn().await?;
        self::update(&mut conn, id, patch).await
    }}
"#,
    );
    out.push_str(&body);
}

fn emit_delete_wrapper(out: &mut String) {
    let body = r#"    /// Auto-conn variant of the module-level delete fn.
    pub async fn delete(id: i64) -> ::std::result::Result<(), crate::meltdown::MeltDown> {
        let mut conn = crate::database::acquire_conn().await?;
        self::delete(&mut conn, id).await
    }
"#;
    out.push_str(body);
}

fn emit_list_from_query_wrapper(out: &mut String, _table: &str, stem: &str) {
    let query_ty = naming::query_type(stem);
    let body = format!(
        r#"    /// Lift a wire ListQuery into the typed builder; HTTP integration helper.
    /// Today returns the unfiltered builder; once state-extensions exposes
    /// per-field FilterKind metadata each `(col, value)` pair will dispatch
    /// to the matching scope method.
    pub fn list_from_query(
        _q: &crate::structs::list_query::ListQuery,
    ) -> {query_ty} {{
        {query_ty}::new()
    }}
"#,
    );
    out.push_str(&body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impl_block_contains_all_wrappers() {
        let mut out = String::new();
        emit_impl_block(&mut out, "users", "User", &VerbSelection::all());
        for needle in [
            "impl User {",
            "pub async fn list(",
            "pub async fn get(id: i64)",
            "pub async fn create(",
            "pub async fn update(",
            "pub async fn delete(id: i64)",
            "pub fn list_from_query(",
        ] {
            assert!(out.contains(needle), "missing: {needle}\n---\n{out}");
        }
    }

    #[test]
    fn wrappers_call_pool_and_super() {
        let mut out = String::new();
        emit_impl_block(&mut out, "users", "User", &VerbSelection::all());
        assert!(out.contains("crate::database::acquire_conn().await"));
        assert!(out.contains("self::list(&mut conn, query).await"));
        assert!(out.contains("self::get(&mut conn, id).await"));
        assert!(out.contains("self::create(&mut conn, input).await"));
        assert!(out.contains("self::update(&mut conn, id, patch).await"));
        assert!(out.contains("self::delete(&mut conn, id).await"));
    }

    #[test]
    fn list_from_query_returns_query_builder() {
        let mut out = String::new();
        emit_impl_block(&mut out, "users", "User", &VerbSelection::all());
        assert!(out.contains("-> UserQuery {"));
        assert!(out.contains("UserQuery::new()"));
    }

    #[test]
    fn skips_create_update_wrappers_when_verbs_off() {
        let mut out = String::new();
        let verbs = VerbSelection { list: true, get: true, create: false, update: false, delete: true };
        emit_impl_block(&mut out, "users", "User", &verbs);
        assert!(!out.contains("pub async fn create("), "create wrapper must not emit:\n{out}");
        assert!(!out.contains("pub async fn update("), "update wrapper must not emit:\n{out}");
    }
}
