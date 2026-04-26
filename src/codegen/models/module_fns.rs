//! Emit the txn-friendly module-level fns.
//!
//! Five fns per resource: list, get, create, update, delete. Each takes
//! an explicit borrowed `AsyncPgConnection` so flows can compose them
//! inside transactions. The auto-conn wrappers in `auto_conn.rs` cover
//! the 95% callsite that just wants a one-shot acquire from the pool.

use crate::codegen::models::naming;
use crate::codegen::models::soft_delete::{self, SoftDeleteConfig};

pub fn emit_all(
    out: &mut String,
    table: &str,
    stem: &str,
    soft_delete_cfg: Option<&SoftDeleteConfig>,
) {
    emit_list(out, table, stem);
    out.push('\n');
    emit_get(out, table, stem);
    out.push('\n');
    emit_create(out, table, stem);
    out.push('\n');
    emit_update(out, table, stem);
    out.push('\n');
    emit_delete(out, table, soft_delete_cfg);
}

fn emit_list(out: &mut String, table: &str, stem: &str) {
    let body = format!(
        r#"/// Page-fetch rows under the locked list contract.
pub async fn list(
    conn: &mut ::diesel_async::AsyncPgConnection,
    query: &::catalyst::transport::http::list_query::ListQuery,
) -> ::std::result::Result<
    ::catalyst::transport::http::list_query::ListResponse<{stem}>,
    ::catalyst::meltdown::MeltDown,
> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;

    let total: i64 = ::diesel::QueryDsl::count(schema::{table})
        .get_result(conn)
        .await?;

    let offset = ((query.page.saturating_sub(1)) as i64) * (query.page_size as i64);
    let limit = query.page_size as i64;
    let mut q = schema::{table}.into_boxed();
    if query.sort.is_empty() {{
        q = ::diesel::QueryDsl::order(q, schema::id.asc());
    }}
    q = ::diesel::QueryDsl::limit(::diesel::QueryDsl::offset(q, offset), limit);

    let items: ::std::vec::Vec<{stem}> = q.load(conn).await?;
    Ok(::catalyst::transport::http::list_query::ListResponse::from_query(
        items, query, total as u64,
    ))
}}
"#,
    );
    out.push_str(&body);
}

fn emit_get(out: &mut String, table: &str, stem: &str) {
    let body = format!(
        r#"/// Fetch a single row by primary key.
pub async fn get(
    conn: &mut ::diesel_async::AsyncPgConnection,
    id: i64,
) -> ::std::result::Result<{stem}, ::catalyst::meltdown::MeltDown> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;
    let row: {stem} = schema::{table}
        .filter(schema::id.eq(id))
        .first(conn)
        .await
        .map_err(|e: ::diesel::result::Error| match e {{
            ::diesel::result::Error::NotFound => {{
                ::catalyst::meltdown::MeltDown::not_found("{table}", id.to_string())
            }}
            other => ::catalyst::meltdown::MeltDown::from(other),
        }})?;
    Ok(row)
}}
"#,
    );
    out.push_str(&body);
}

fn emit_create(out: &mut String, table: &str, stem: &str) {
    let insertable = naming::insertable_type(stem);
    let body = format!(
        r#"/// Insert a new row, returning the inserted record.
pub async fn create(
    conn: &mut ::diesel_async::AsyncPgConnection,
    input: &{insertable},
) -> ::std::result::Result<{stem}, ::catalyst::meltdown::MeltDown> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;
    let row: {stem} = ::diesel::insert_into(schema::{table})
        .values(input)
        .get_result(conn)
        .await?;
    Ok(row)
}}
"#,
    );
    out.push_str(&body);
}

fn emit_update(out: &mut String, table: &str, stem: &str) {
    let patch = naming::patch_type(stem);
    let body = format!(
        r#"/// Patch an existing row by primary key.
pub async fn update(
    conn: &mut ::diesel_async::AsyncPgConnection,
    id: i64,
    patch: &{patch},
) -> ::std::result::Result<{stem}, ::catalyst::meltdown::MeltDown> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;
    let row: {stem} = ::diesel::update(schema::{table}.filter(schema::id.eq(id)))
        .set(patch)
        .get_result(conn)
        .await
        .map_err(|e: ::diesel::result::Error| match e {{
            ::diesel::result::Error::NotFound => {{
                ::catalyst::meltdown::MeltDown::not_found("{table}", id.to_string())
            }}
            other => ::catalyst::meltdown::MeltDown::from(other),
        }})?;
    Ok(row)
}}
"#,
    );
    out.push_str(&body);
}

fn emit_delete(out: &mut String, table: &str, soft_delete_cfg: Option<&SoftDeleteConfig>) {
    let header = "/// Delete a row by primary key. Returns MeltDown::not_found when no row is affected.\n";
    let signature = "pub async fn delete(\n    conn: &mut ::diesel_async::AsyncPgConnection,\n    id: i64,\n) -> ::std::result::Result<(), ::catalyst::meltdown::MeltDown> {\n";
    out.push_str(header);
    out.push_str(signature);
    match soft_delete_cfg {
        Some(cfg) => {
            let mut body = String::new();
            soft_delete::emit_delete_body(&mut body, table, cfg);
            out.push_str(&body);
        }
        None => {
            let body = format!(
                r#"    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;
    let n = ::diesel::delete(schema::{table}.filter(schema::id.eq(id)))
        .execute(conn)
        .await?;
    if n == 0 {{
        return Err(::catalyst::meltdown::MeltDown::not_found(
            "{table}",
            id.to_string(),
        ));
    }}
    Ok(())
"#,
            );
            out.push_str(&body);
        }
    }
    out.push_str("}\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_uses_explicit_conn_and_diesel_typed_builder() {
        let mut out = String::new();
        emit_list(&mut out, "users", "User");
        assert!(out.contains("pub async fn list("));
        assert!(out.contains("conn: &mut ::diesel_async::AsyncPgConnection"));
        assert!(out.contains("query: &::catalyst::transport::http::list_query::ListQuery"));
        assert!(out.contains("ListResponse<User>"));
        assert!(out.contains("schema::id.asc()"));
        assert!(out.contains("ListResponse::from_query"));
    }

    #[test]
    fn get_maps_notfound_to_meltdown_not_found() {
        let mut out = String::new();
        emit_get(&mut out, "users", "User");
        assert!(out.contains("Error::NotFound"));
        assert!(out.contains("MeltDown::not_found(\"users\""));
    }

    #[test]
    fn create_uses_returning_via_get_result() {
        let mut out = String::new();
        emit_create(&mut out, "users", "User");
        assert!(out.contains("input: &UserInsertable"));
        assert!(out.contains("insert_into("));
        assert!(out.contains(".get_result("));
    }

    #[test]
    fn update_takes_patch_and_maps_notfound() {
        let mut out = String::new();
        emit_update(&mut out, "users", "User");
        assert!(out.contains("patch: &UserPatch"));
        assert!(out.contains("Error::NotFound"));
    }

    #[test]
    fn delete_hard_emits_not_found_on_zero_rows() {
        let mut out = String::new();
        emit_delete(&mut out, "users", None);
        assert!(out.contains("::diesel::delete("));
        assert!(out.contains("if n == 0"));
        assert!(out.contains("MeltDown::not_found(\n            \"users\""));
    }

    #[test]
    fn delete_soft_uses_update() {
        use crate::codegen::models::soft_delete::DefaultBehavior;
        let mut out = String::new();
        emit_delete(
            &mut out,
            "users",
            Some(&SoftDeleteConfig {
                column: "deleted_at".to_string(),
                default_behavior: DefaultBehavior::Exclude,
            }),
        );
        assert!(out.contains("::diesel::update("));
        assert!(!out.contains("::diesel::delete("));
    }

    #[test]
    fn emit_all_emits_all_five_fns() {
        let mut out = String::new();
        emit_all(&mut out, "users", "User", None);
        for sig in [
            "pub async fn list(",
            "pub async fn get(",
            "pub async fn create(",
            "pub async fn update(",
            "pub async fn delete(",
        ] {
            assert!(out.contains(sig), "module_fns missing: {sig}\n{out}");
        }
    }
}
