//! TableRow emitter — display-safe projection of the Public variant.
//!
//! Emits a plain struct holding only schema fields whose Rust type has a
//! Display impl in our type map. Jsonb, Bytea, Numeric and Decimal are
//! skipped so the row struct can be rendered by leptos `view!` directly.
//! The list page iterates the row vec with native `<For>` — no third-party
//! table crate involved.
//!
//! A From-Public impl is emitted so the list page can convert Public
//! projections into row values in one move.
//!
//! Gating: emission is controlled by `gen_level >= Components` at the runner.

use super::util;
use crate::{
    codegen::structs::{naming, sql_map},
    state::{FieldName, FieldState, FieldVariant, ResourceState, SqlType},
};

pub fn render(resource: &ResourceState) -> String {
    let public_name = naming::struct_name_for_variant_resource(resource, FieldVariant::Public);
    let row_name = format!("{}TableRow", naming::type_stem_for_resource(resource));

    let display_fields: Vec<(&FieldName, &FieldState)> = util::fields_for_variant(resource, FieldVariant::Public).into_iter().filter(|(_, field)| is_display_safe(&field.sql_type)).collect();

    let mut out = String::new();
    out.push_str("#[derive(Debug, Clone, ::serde::Serialize)]\n");
    out.push_str(&format!("pub struct {row_name} {{\n"));
    for (name, field) in &display_fields {
        let ty = sql_map::rust_type(&field.sql_type, field.nullable);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n\n");

    out.push_str(&format!("impl From<{public_name}> for {row_name} {{\n"));
    out.push_str(&format!("    fn from(row: {public_name}) -> Self {{\n"));
    out.push_str("        Self {\n");
    for (name, _field) in &display_fields {
        let n = name.as_str();
        out.push_str(&format!("            {n}: row.{n},\n"));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

/// Whether a SQL type maps to a Rust type with a `core::fmt::Display` impl
/// usable from leptos `view!` macros. Excludes Jsonb/Bytea/Numeric/Decimal —
/// either no Display impl or the type isn't in the canonical feature set.
///
/// Unknown SQL types fall back to String in the Rust mapper, which IS
/// display-safe — treated as such here.
pub fn is_display_safe(sql: &SqlType) -> bool {
    match sql.as_str().to_ascii_lowercase().as_str() {
        "json" | "jsonb" | "bytea" | "numeric" | "decimal" => false,
        _other => true,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;

    use super::*;
    use crate::state::{names::ResourceName, AuthMode, FilterKind, ListOptions, Verb, VerbState};

    fn variants(items: &[FieldVariant]) -> BTreeSet<FieldVariant> {
        items.iter().copied().collect()
    }

    fn field(sql: &str, vs: &[FieldVariant], nullable: bool, pk: bool) -> FieldState {
        FieldState {
            sql_type: SqlType::new(sql),
            variants: variants(vs),
            nullable,
            primary_key: pk,
            validators: BTreeSet::new(),
            kind: Default::default(),
        }
    }

    fn full_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(FieldName::new("id"), field("Int8", &[FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin], false, true));
        fields.insert(
            FieldName::new("title"),
            field("Varchar", &[FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public, FieldVariant::Admin], false, false),
        );
        fields.insert(FieldName::new("payload"), field("Jsonb", &[FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin], false, false));
        fields.insert(FieldName::new("blob"), field("Bytea", &[FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin], true, false));
        fields.insert(FieldName::new("created_at"), field("Timestamptz", &[FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin], false, false));

        let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
        filterable.insert(FieldName::new("title"), FilterKind::IlikeContains);
        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: filterable,
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: Some(100),
                }),
                emit_rest_api: true,
                emit_html_page: true,
            },
        );

        let mut resource = ResourceState::new(ResourceName::new("posts"));
        resource.fields = fields;
        resource.verbs = verbs;
        resource.canonicalize();
        resource
    }

    #[test]
    fn emits_plain_struct_no_external_derive() {
        let resource = full_resource();
        let body = render(&resource);
        assert!(body.contains("#[derive(Debug, Clone, ::serde::Serialize)]"), "must derive Serialize for TableBuilder consumption:\n{body}");
        assert!(!body.contains("leptos_struct_table"), "no external table-crate derive should leak in:\n{body}");
        assert!(body.contains("pub struct PostTableRow {"));
    }

    #[test]
    fn skips_jsonb_and_bytea_fields() {
        let resource = full_resource();
        let body = render(&resource);
        assert!(!body.contains("pub payload:"), "Jsonb field must be skipped:\n{body}");
        assert!(!body.contains("pub blob:"), "Bytea field must be skipped:\n{body}");
    }

    #[test]
    fn keeps_display_safe_fields() {
        let resource = full_resource();
        let body = render(&resource);
        assert!(body.contains("pub id: i64"));
        assert!(body.contains("pub title: String"));
        assert!(body.contains("pub created_at: chrono::DateTime<chrono::Utc>"));
    }

    #[test]
    fn emits_from_public_impl() {
        let resource = full_resource();
        let body = render(&resource);
        assert!(body.contains("impl From<PostPublic> for PostTableRow"));
        assert!(body.contains("id: row.id"));
        assert!(body.contains("title: row.title"));
        assert!(body.contains("created_at: row.created_at"));
        assert!(!body.contains("payload: row.payload"));
        assert!(!body.contains("blob: row.blob"));
    }

    #[test]
    fn singular_override_propagates_to_row_name() {
        let mut resource = full_resource();
        resource.name = ResourceName::new("data");
        resource.singular_override = Some("datum".to_string());
        let body = render(&resource);
        assert!(body.contains("pub struct DatumTableRow {"));
        assert!(body.contains("impl From<DatumPublic> for DatumTableRow"));
    }

    #[test]
    fn is_display_safe_recognizes_unsafe_sql_types() {
        assert!(!is_display_safe(&SqlType::new("Jsonb")));
        assert!(!is_display_safe(&SqlType::new("Json")));
        assert!(!is_display_safe(&SqlType::new("Bytea")));
        assert!(!is_display_safe(&SqlType::new("Numeric")));
        assert!(!is_display_safe(&SqlType::new("Decimal")));
    }

    #[test]
    fn is_display_safe_keeps_common_types() {
        for sql in ["Int8", "Int4", "Int2", "Bool", "Varchar", "Text", "Timestamptz", "Date", "Uuid", "Float8"] {
            assert!(is_display_safe(&SqlType::new(sql)), "{} should be display-safe", sql);
        }
    }
}
