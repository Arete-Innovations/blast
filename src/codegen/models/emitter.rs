//! Per-resource model emitter.
//!
//! Composes module-level fns + auto-conn `impl <Type>` wrappers + the
//! fluent `<Type>Query` builder + paginated terminal + IntoFuture impls
//! into a single Rust file body. The runner prepends the codegen marker
//! and writes it to `src/models/generated/<table>.rs`.

use crate::{
    codegen::models::{aggregations, auto_conn, builder, eager::Relation, module_fns, naming, soft_delete::SoftDeleteConfig},
    state::ResourceState,
};

/// Render a full per-resource file body (no marker — caller prepends).
pub fn render_resource_body(resource: &ResourceState, relations: &[Relation], soft_delete: Option<&SoftDeleteConfig>) -> String {
    let table = resource.name.as_str();
    let stem = naming::type_stem_for(resource);

    let mut out = String::new();

    out.push_str(&imports_block(table, &stem));
    out.push('\n');

    module_fns::emit_all(&mut out, table, &stem, soft_delete);
    out.push('\n');

    auto_conn::emit_impl_block(&mut out, table, &stem);
    out.push('\n');

    out.push_str(&format!("impl {stem} {{\n"));
    aggregations::emit_type_count(&mut out, table, &stem);
    out.push_str("}\n\n");

    builder::emit(&mut out, resource, relations, soft_delete);

    out
}

fn imports_block(table: &str, stem: &str) -> String {
    let insertable = naming::insertable_type(stem);
    let patch = naming::patch_type(stem);
    let mut out = String::new();
    out.push_str("#![allow(unused_imports, dead_code, clippy::needless_borrow)]\n\n");
    out.push_str("use ::diesel::{BoolExpressionMethods, ExpressionMethods, JoinOnDsl, NullableExpressionMethods, PgTextExpressionMethods, QueryDsl};\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{stem}, {insertable}, {patch}}};\n",));
    out
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

    fn resource_with_columns() -> ResourceState {
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
        fields.insert(
            FieldName::new("email"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: variants(&[FieldVariant::Db, FieldVariant::Public]),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("author_id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: variants(&[FieldVariant::Db]),
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
                    max_page_size: None,
                }),
                emit_rest_api: true,
                emit_html_page: true,
            },
        );
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );

        let mut r = ResourceState::new(ResourceName::new("users"));
        r.fields = fields;
        r.verbs = verbs;
        r
    }

    #[test]
    fn body_includes_all_module_fns_and_impl_block() {
        let body = render_resource_body(&resource_with_columns(), &[], None);
        for needle in ["pub async fn list(", "pub async fn get(", "pub async fn create(", "pub async fn update(", "pub async fn delete(", "impl User {"] {
            assert!(body.contains(needle), "missing {needle}\n---\n{body}");
        }
    }

    #[test]
    fn body_includes_query_builder_and_into_future() {
        let body = render_resource_body(&resource_with_columns(), &[], None);
        assert!(body.contains("pub struct UserQuery"));
        assert!(body.contains("impl ::std::future::IntoFuture for UserQuery"));
        assert!(body.contains("pub struct UserQueryPaginated"));
        assert!(body.contains("impl ::std::future::IntoFuture for UserQueryPaginated"));
    }

    #[test]
    fn auto_conn_wrappers_use_pool() {
        let body = render_resource_body(&resource_with_columns(), &[], None);
        assert!(body.contains("crate::database::acquire_conn().await"));
    }

    #[test]
    fn auto_derived_scope_emitted_for_each_filter_kind() {
        let body = render_resource_body(&resource_with_columns(), &[], None);
        // Bool
        assert!(body.contains("pub fn active(mut self) -> Self"));
        assert!(body.contains("pub fn not_active(mut self) -> Self"));
        // TimestampInt64
        assert!(body.contains("pub fn before_created_at("));
        assert!(body.contains("pub fn created_at_today"));
        // Text
        assert!(body.contains("pub fn where_email_contains"));
        // Int + FK
        assert!(body.contains("pub fn where_author_id_eq"));
        assert!(body.contains("pub fn by_author"));
    }

    #[test]
    fn soft_delete_scopes_emitted_when_declared() {
        let cfg = SoftDeleteConfig {
            column: "deleted_at".to_string(),
            default_behavior: DefaultBehavior::Exclude,
        };
        let body = render_resource_body(&resource_with_columns(), &[], Some(&cfg));
        assert!(body.contains("pub fn with_deleted(mut self)"));
        assert!(body.contains("pub fn exclude_deleted(mut self)"));
        assert!(body.contains("pub fn only_deleted(mut self)"));
        // delete fn was redirected to UPDATE.
        assert!(body.contains("::diesel::update("));
    }

    #[test]
    fn eager_loader_scope_emitted_per_relation() {
        let rels = vec![Relation {
            rel_name: "author".to_string(),
            fk_column: "author_id".to_string(),
            target_table: "users".to_string(),
        }];
        let body = render_resource_body(&resource_with_columns(), &rels, None);
        assert!(body.contains("pub fn with_author(mut self) -> Self"));
        assert!(body.contains("pub(crate) with_author: bool,"));
    }

    #[test]
    fn aggregations_count_exists_first() {
        let body = render_resource_body(&resource_with_columns(), &[], None);
        // type-level count
        assert!(body.contains("impl User {"));
        assert!(body.contains("pub async fn count() ->"));
        // builder-level
        assert!(body.contains("pub async fn count(self)"));
        assert!(body.contains("pub async fn exists(self)"));
        assert!(body.contains("pub async fn first("));
    }
}
