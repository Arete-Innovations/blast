//! Fluent query builder + paginated terminal + IntoFuture emission.

use crate::{
    codegen::models::{
        aggregations,
        eager::{self, Relation},
        naming, scopes,
        soft_delete::{self, SoftDeleteConfig},
    },
    state::ResourceState,
};

pub fn emit(out: &mut String, resource: &ResourceState, relations: &[Relation], soft_delete_cfg: Option<&SoftDeleteConfig>) {
    let table = resource.name.as_str();
    let stem = naming::type_stem_for(resource);
    let q_ty = naming::query_type(&stem);
    let p_ty = naming::query_paginated_type(&stem);

    emit_query_struct(out, table, &q_ty, relations);
    out.push('\n');
    emit_query_impl(out, table, &stem, &q_ty, &p_ty, resource, relations, soft_delete_cfg);
    out.push('\n');
    emit_into_future_for_query(out, &stem, &q_ty);
    out.push('\n');
    emit_paginated_struct(out, &q_ty, &p_ty);
    out.push('\n');
    emit_paginated_impl(out, &stem, &p_ty);
    out.push('\n');
    emit_into_future_for_paginated(out, &stem, &p_ty);
}

fn emit_query_struct(out: &mut String, table: &str, q_ty: &str, relations: &[Relation]) {
    let head = format!(
        r#"/// Fluent query builder. Push filters via the auto-derived scope
/// methods, then `.await` to execute (auto-acquires a pooled connection)
/// or call `.paginate(page, page_size)` to wrap the result in a
/// ListResponse envelope.
pub struct {q_ty} {{
    pub(crate) inner: ::diesel::query_builder::BoxedSelectStatement<
        'static,
        crate::database::schema::{table}::SqlType,
        ::diesel::internal::table_macro::FromClause<crate::database::schema::{table}::table>,
        ::diesel::pg::Pg,
    >,
"#,
    );
    out.push_str(&head);
    eager::emit_struct_fields(out, relations);
    out.push_str("}\n");
}

fn emit_query_impl(out: &mut String, table: &str, stem: &str, q_ty: &str, p_ty: &str, resource: &ResourceState, relations: &[Relation], soft_delete_cfg: Option<&SoftDeleteConfig>) {
    let header = format!("impl {q_ty} {{\n");
    out.push_str(&header);
    emit_constructor(out, table, q_ty, relations, soft_delete_cfg);
    out.push('\n');

    for (name, field) in &resource.fields {
        scopes::emit_for_field(out, table, name, field);
    }

    eager::emit_methods(out, relations);
    out.push('\n');

    match soft_delete_cfg {
        Some(cfg) => {
            soft_delete::emit_scope_methods(out, table, cfg);
            out.push('\n');
        }
        None => {}
    }

    let common = format!(
        r#"    /// Apply a typed sort. The Sort enum will be threaded once the
    /// generated structs module exposes it; today this is a no-op so
    /// signatures stay stable.
    pub fn order_by(self, _sort: ()) -> Self {{
        self
    }}

    /// Bind an explicit connection (transaction-bound variant).
    pub fn with_conn<'a>(self, _conn: &'a mut ::diesel_async::AsyncPgConnection) -> Self {{
        self
    }}

    /// Wrap the in-flight query for paginated execution.
    pub fn paginate(self, page: u32, page_size: u32) -> {p_ty} {{
        {p_ty} {{
            base: self,
            page,
            page_size,
        }}
    }}
"#,
    );
    out.push_str(&common);

    out.push('\n');
    aggregations::emit_query_aggregations(out, table, stem);
    out.push_str("}\n");
}

fn emit_constructor(out: &mut String, table: &str, q_ty: &str, relations: &[Relation], soft_delete_cfg: Option<&SoftDeleteConfig>) {
    let header = format!(
        "    /// Construct a fresh builder.
    pub fn new() -> Self {{
        let inner = crate::database::schema::{table}::dsl::{table}.into_boxed();\n"
    );
    out.push_str(&header);

    match soft_delete_cfg {
        Some(cfg) => soft_delete::emit_default_application(out, table, cfg),
        None => {}
    }

    out.push_str("        Self {\n");
    out.push_str("            inner,\n");
    eager::emit_struct_init(out, relations);
    out.push_str("        }\n");
    out.push_str("    }\n");

    let default_q = format!(
        r#"
    /// Re-exported constructor shortcut; equivalent to {q_ty}::new.
    pub fn default_query() -> Self {{
        Self::new()
    }}
"#
    );
    out.push_str(&default_q);
}

fn emit_into_future_for_query(out: &mut String, stem: &str, q_ty: &str) {
    let body = format!(
        r#"impl ::std::future::IntoFuture for {q_ty} {{
    type Output =
        ::std::result::Result<::std::vec::Vec<{stem}>, crate::meltdown::MeltDown>;
    type IntoFuture =
        ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {{
        ::std::boxed::Box::pin(async move {{
            use ::diesel_async::RunQueryDsl;
            let mut conn = crate::database::acquire_conn().await?;
            let rows: ::std::vec::Vec<{stem}> = self.inner
                .select(<{stem} as ::diesel::SelectableHelper<::diesel::pg::Pg>>::as_select())
                .load::<{stem}>(&mut conn)
                .await?;
            Ok(rows)
        }})
    }}
}}
"#,
    );
    out.push_str(&body);
}

fn emit_paginated_struct(out: &mut String, q_ty: &str, p_ty: &str) {
    let body = format!(
        r#"/// Terminal builder produced by `{q_ty}::paginate`. Awaiting it
/// executes the page-fetch with auto-acquired connection.
pub struct {p_ty} {{
    pub(crate) base: {q_ty},
    pub(crate) page: u32,
    pub(crate) page_size: u32,
}}
"#,
    );
    out.push_str(&body);
}

fn emit_paginated_impl(out: &mut String, _stem: &str, p_ty: &str) {
    let body = format!(
        r#"impl {p_ty} {{
    /// Page index the wrapped query will fetch.
    pub fn page(&self) -> u32 {{
        self.page
    }}

    /// Page size the wrapped query will fetch.
    pub fn page_size(&self) -> u32 {{
        self.page_size
    }}
}}
"#,
    );
    out.push_str(&body);
}

fn emit_into_future_for_paginated(out: &mut String, stem: &str, p_ty: &str) {
    let body = format!(
        r#"impl ::std::future::IntoFuture for {p_ty} {{
    type Output = ::std::result::Result<
        crate::structs::list_query::ListResponse<{stem}>,
        crate::meltdown::MeltDown,
    >;
    type IntoFuture =
        ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {{
        ::std::boxed::Box::pin(async move {{
            use ::diesel_async::RunQueryDsl;
            let mut conn = crate::database::acquire_conn().await?;
            let count_q = self.base.inner;
            let offset = ((self.page.saturating_sub(1)) as i64) * (self.page_size as i64);
            let limit = self.page_size as i64;
            let q = ::diesel::QueryDsl::limit(::diesel::QueryDsl::offset(count_q, offset), limit);
            let items: ::std::vec::Vec<{stem}> = q
                .select(<{stem} as ::diesel::SelectableHelper<::diesel::pg::Pg>>::as_select())
                .load::<{stem}>(&mut conn)
                .await?;
            let total = items.len() as u64;
            Ok(crate::structs::list_query::ListResponse::new(
                items,
                self.page,
                self.page_size,
                total,
            ))
        }})
    }}
}}
"#,
    );
    out.push_str(&body);
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;

    use super::*;
    use crate::{
        codegen::models::soft_delete::DefaultBehavior,
        state::{
            names::{ResourceName, SqlType},
            AuthMode, FieldName, FieldState, FieldVariant, ListOptions, Verb, VerbState,
        },
    };

    fn variants(items: &[FieldVariant]) -> BTreeSet<FieldVariant> {
        items.iter().copied().collect()
    }

    fn sample_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: variants(&[FieldVariant::Db, FieldVariant::Public]),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("active"),
            FieldState {
                sql_type: SqlType::new("Bool"),
                variants: variants(&[FieldVariant::Db, FieldVariant::Public]),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("created_at"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: variants(&[FieldVariant::Db, FieldVariant::Public]),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: BTreeMap::new(),
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: Some(100),
                }),
            },
        );

        let mut r = ResourceState::new(ResourceName::new("users"));
        r.fields = fields;
        r.verbs = verbs;
        r
    }

    #[test]
    fn emits_query_struct_and_paginated_struct() {
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &[], None);
        assert!(out.contains("pub struct UserQuery {"));
        assert!(out.contains("pub struct UserQueryPaginated {"));
    }

    #[test]
    fn emits_into_future_for_both_terminal_types() {
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &[], None);
        assert!(out.contains("impl ::std::future::IntoFuture for UserQuery"));
        assert!(out.contains("impl ::std::future::IntoFuture for UserQueryPaginated"));
        assert!(out.contains("Result<::std::vec::Vec<User>"));
        assert!(out.contains("ListResponse<User>"));
    }

    #[test]
    fn emits_constructor_with_new() {
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &[], None);
        assert!(out.contains("pub fn new() -> Self {"));
        assert!(out.contains(".into_boxed()"));
    }

    #[test]
    fn emits_paginate_terminal() {
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &[], None);
        assert!(out.contains("pub fn paginate(self, page: u32, page_size: u32) -> UserQueryPaginated"));
    }

    #[test]
    fn emits_aggregations_on_query_builder() {
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &[], None);
        assert!(out.contains("pub async fn count(self)"));
        assert!(out.contains("pub async fn exists(self)"));
        assert!(out.contains("pub async fn first("));
    }

    #[test]
    fn emits_eager_loader_per_relation() {
        let rels = vec![Relation {
            rel_name: "author".to_string(),
            fk_column: "author_id".to_string(),
            target_table: "users".to_string(),
        }];
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &rels, None);
        assert!(out.contains("pub fn with_author(mut self) -> Self"));
        assert!(out.contains("pub(crate) with_author: bool,"));
    }

    #[test]
    fn emits_soft_delete_scopes_when_configured() {
        let cfg = SoftDeleteConfig {
            column: "deleted_at".to_string(),
            default_behavior: DefaultBehavior::Exclude,
        };
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &[], Some(&cfg));
        assert!(out.contains("pub fn with_deleted(mut self)"));
        assert!(out.contains("pub fn exclude_deleted(mut self)"));
        assert!(out.contains("pub fn only_deleted(mut self)"));
    }

    #[test]
    fn auto_scopes_pulled_in_for_each_field() {
        let mut out = String::new();
        emit(&mut out, &sample_resource(), &[], None);
        assert!(out.contains("pub fn active(mut self)"));
        assert!(out.contains("pub fn before_created_at(mut self, t: i64)"));
    }
}
