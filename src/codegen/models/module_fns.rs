//! Emit the txn-friendly module-level fns.
//!
//! Five fns per resource: list, get, create, update, delete. Each takes
//! an explicit borrowed `AsyncPgConnection` so flows can compose them
//! inside transactions. The auto-conn wrappers in `auto_conn.rs` cover
//! the 95% callsite that just wants a one-shot acquire from the pool.

use crate::{
    codegen::models::{
        naming,
        soft_delete::{self, SoftDeleteConfig},
    },
    state::{FilterKind, ResourceState, Verb},
};

pub fn emit_all(out: &mut String, resource: &ResourceState, soft_delete_cfg: Option<&SoftDeleteConfig>, verbs: &VerbSelection) {
    let table = resource.name.as_str();
    let stem = naming::type_stem_for(resource);
    if verbs.list {
        emit_list(out, resource, table, &stem);
        out.push('\n');
    }
    if verbs.get {
        emit_get(out, table, &stem);
        out.push('\n');
    }
    if verbs.create {
        emit_create(out, table, &stem);
        out.push('\n');
    }
    if verbs.update {
        emit_update(out, table, &stem);
        out.push('\n');
    }
    if verbs.delete {
        emit_delete(out, table, soft_delete_cfg);
    }
}

fn emit_filter_match_arms(resource: &ResourceState) -> String {
    let opts = match resource.verbs.get(&Verb::List).and_then(|v| v.list_options.as_ref()) {
        Some(o) => o,
        None => return String::new(),
    };
    if opts.filterable_columns.is_empty() {
        return String::new();
    }
    let mut arms = String::new();
    for (col, kind) in &opts.filterable_columns {
        let col_name = col.as_str();
        let sql = match resource.fields.get(col) {
            Some(f) => f.sql_type.as_str().to_ascii_lowercase(),
            None => continue,
        };
        let clause = render_filter_clause(col_name, &sql, *kind);
        match clause {
            Some(c) => arms.push_str(&format!("            \"{col_name}\" => {{ {c} }}\n")),
            None => continue,
        }
    }
    if arms.is_empty() {
        return String::new();
    }
    format!(
        "    for (k, v) in &query.filter {{\n        match k.as_str() {{\n{arms}            other => return Err(crate::meltdown::MeltDown::bad_request(format!(\"unknown filter column: {{}}\", other))),\n        }}\n    }}\n"
    )
}

fn render_filter_clause(col: &str, sql: &str, kind: FilterKind) -> Option<String> {
    let is_text = matches!(sql, "text" | "varchar" | "bpchar" | "char" | "citext");
    let int_ty: Option<&str> = match sql {
        "int2" | "smallint" | "smallserial" => Some("i16"),
        "int4" | "integer" | "serial" => Some("i32"),
        "int8" | "bigint" | "bigserial" => Some("i64"),
        non_int => { let _ = non_int; None } // allow: non-integer sql_type — caller handles via match below
    };
    match (kind, is_text, int_ty) {
        (FilterKind::Eq, true, _) => Some(format!("q = q.filter(schema::{col}.eq(v.clone()));")),
        (FilterKind::Eq, false, Some(ty)) => Some(format!("match v.parse::<{ty}>() {{ Ok(n) => q = q.filter(schema::{col}.eq(n)), Err(e) => return Err(crate::meltdown::MeltDown::bad_request(format!(\"filter {col}: {{}}\", e))) }}")),
        (FilterKind::Eq, false, None) => None,
        (FilterKind::IlikeContains, true, _) => Some(format!("q = q.filter(schema::{col}.ilike(format!(\"%{{}}%\", v)));")),
        (FilterKind::IlikeContains, false, _) => None,
        (FilterKind::Range, _, _) => None,
        (FilterKind::In, _, _) => None,
        (FilterKind::Bool, _, _) => None,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VerbSelection {
    pub list: bool,
    pub get: bool,
    pub create: bool,
    pub update: bool,
    pub delete: bool,
}

impl VerbSelection {
    pub fn from_resource(verbs: &::indexmap::IndexMap<Verb, crate::state::VerbState>) -> Self {
        Self {
            list: verbs.contains_key(&Verb::List),
            get: verbs.contains_key(&Verb::Get),
            create: verbs.contains_key(&Verb::Create),
            update: verbs.contains_key(&Verb::Update),
            delete: verbs.contains_key(&Verb::Delete),
        }
    }

    pub fn all() -> Self {
        Self { list: true, get: true, create: true, update: true, delete: true }
    }
}

fn emit_list(out: &mut String, resource: &ResourceState, table: &str, stem: &str) {
    let filter_block = emit_filter_match_arms(resource);
    let body = format!(
        r#"pub async fn list(
    conn: &mut ::diesel_async::AsyncPgConnection,
    query: &crate::structs::list_query::ListQuery,
) -> ::std::result::Result<
    crate::structs::list_query::ListResponse<{stem}>,
    crate::meltdown::MeltDown,
> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;

    let offset = ((query.page.saturating_sub(1)) as i64) * (query.page_size as i64);
    let limit = query.page_size as i64;
    let mut q = schema::{table}.into_boxed();
{filter_block}    if query.sort.is_empty() {{
        q = ::diesel::QueryDsl::order(q, schema::id.asc());
    }}

    let mut count_q = schema::{table}.into_boxed();
{filter_block_count}    let total: i64 = ::diesel::QueryDsl::count(count_q)
        .get_result::<i64>(conn)
        .await?;

    q = ::diesel::QueryDsl::limit(::diesel::QueryDsl::offset(q, offset), limit);

    let items: ::std::vec::Vec<{stem}> = q
        .select(<{stem} as ::diesel::SelectableHelper<::diesel::pg::Pg>>::as_select())
        .load::<{stem}>(conn)
        .await?;
    Ok(crate::structs::list_query::ListResponse::from_query(
        items, query, total as u64,
    ))
}}
"#,
        filter_block_count = filter_block.replace("q = q.filter", "count_q = count_q.filter"),
    );
    out.push_str(&body);
}

fn emit_get(out: &mut String, table: &str, stem: &str) {
    let body = format!(
        r#"pub async fn get(
    conn: &mut ::diesel_async::AsyncPgConnection,
    id: i64,
) -> ::std::result::Result<{stem}, crate::meltdown::MeltDown> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;
    let row: {stem} = schema::{table}
        .filter(schema::id.eq(id))
        .select(<{stem} as ::diesel::SelectableHelper<::diesel::pg::Pg>>::as_select())
        .first::<{stem}>(conn)
        .await
        .map_err(|e: ::diesel::result::Error| match e {{
            ::diesel::result::Error::NotFound => {{
                crate::meltdown::MeltDown::not_found("{table}", id.to_string())
            }}
            other => crate::meltdown::MeltDown::from(other),
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
        r#"pub async fn create(
    conn: &mut ::diesel_async::AsyncPgConnection,
    input: &{insertable},
) -> ::std::result::Result<{stem}, crate::meltdown::MeltDown> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;
    let row: {stem} = ::diesel::insert_into(schema::{table})
        .values(input)
        .returning(<{stem} as ::diesel::SelectableHelper<::diesel::pg::Pg>>::as_select())
        .get_result::<{stem}>(conn)
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
        r#"pub async fn update(
    conn: &mut ::diesel_async::AsyncPgConnection,
    id: i64,
    patch: &{patch},
) -> ::std::result::Result<{stem}, crate::meltdown::MeltDown> {{
    use ::diesel_async::RunQueryDsl;
    use crate::database::schema::{table}::dsl as schema;
    let row: {stem} = ::diesel::update(schema::{table}.filter(schema::id.eq(id)))
        .set(patch)
        .returning(<{stem} as ::diesel::SelectableHelper<::diesel::pg::Pg>>::as_select())
        .get_result::<{stem}>(conn)
        .await
        .map_err(|e: ::diesel::result::Error| match e {{
            ::diesel::result::Error::NotFound => {{
                crate::meltdown::MeltDown::not_found("{table}", id.to_string())
            }}
            other => crate::meltdown::MeltDown::from(other),
        }})?;
    Ok(row)
}}
"#,
    );
    out.push_str(&body);
}

fn emit_delete(out: &mut String, table: &str, soft_delete_cfg: Option<&SoftDeleteConfig>) {
    let signature = "pub async fn delete(\n    conn: &mut ::diesel_async::AsyncPgConnection,\n    id: i64,\n) -> ::std::result::Result<(), crate::meltdown::MeltDown> {\n";
    out.push_str(signature);
    match soft_delete_cfg {
        Some(cfg) => {
            let mut body = String::new();
            soft_delete::emit_delete_body(&mut body, table, cfg);
            out.push_str(&body);
        }
        None => {
            let body = format!(
                r#"    use crate::database::schema::{table}::dsl as schema;
    let n: usize = ::diesel_async::RunQueryDsl::execute(
        ::diesel::delete(schema::{table}.filter(schema::id.eq(id))),
        conn,
    )
    .await?;
    if n == 0 {{
        return Err(crate::meltdown::MeltDown::not_found(
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
    use crate::state::{
        AuthMode,
        CrankPolicy,
        ListOptions,
        VerbState,
        names::ResourceName,
    };
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn users_resource() -> ResourceState {
        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(Verb::List, VerbState {
            auth: AuthMode::Public,
            list_options: Some(ListOptions {
                paginated: true,
                filterable_columns: BTreeMap::new(),
                sortable_columns: BTreeSet::new(),
                default_sort: None,
                max_page_size: None,
            }),
            emit_rest_api: true,
            emit_html_page: true,
                    crank_policy: CrankPolicy::None,
        });
        for v in [Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
            verbs.insert(v, VerbState {
                auth: AuthMode::Public,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
                            crank_policy: CrankPolicy::None,
            });
        }
        let mut r = ResourceState::new(ResourceName::new("users"));
        r.verbs = verbs;
        r
    }

    #[test]
    fn list_uses_explicit_conn_and_diesel_typed_builder() {
        let mut out = String::new();
        let r = users_resource();
        emit_list(&mut out, &r, "users", "User");
        assert!(out.contains("pub async fn list("));
        assert!(out.contains("conn: &mut ::diesel_async::AsyncPgConnection"));
        assert!(out.contains("query: &crate::structs::list_query::ListQuery"));
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
        assert!(out.contains(".get_result::<User>("));
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
        let r = users_resource();
        emit_all(&mut out, &r, None, &VerbSelection::all());
        for sig in ["pub async fn list(", "pub async fn get(", "pub async fn create(", "pub async fn update(", "pub async fn delete("] {
            assert!(out.contains(sig), "module_fns missing: {sig}\n{out}");
        }
    }

    #[test]
    fn emit_all_skips_unselected_verbs() {
        let mut out = String::new();
        let verbs = VerbSelection { list: true, get: true, create: false, update: false, delete: true };
        let r = users_resource();
        emit_all(&mut out, &r, None, &verbs);
        assert!(out.contains("pub async fn list("));
        assert!(out.contains("pub async fn get("));
        assert!(out.contains("pub async fn delete("));
        assert!(!out.contains("pub async fn create("), "create must not emit when verb off:\n{out}");
        assert!(!out.contains("pub async fn update("), "update must not emit when verb off:\n{out}");
    }
}
