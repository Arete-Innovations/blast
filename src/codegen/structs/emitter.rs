//! Per-resource projection-struct emitter.
//!
//! Given a `ResourceState` it produces the body of a single Rust file under
//! `src/structs/generated/<table>.rs`. The body holds:
//!
//! - the base `<Type>` Diesel `Queryable` row (only when the state declares
//!   any `Db` field)
//! - one struct per `FieldVariant` actually present in the state
//!   (`Insertable`, `Patch`, `Public`, `Admin`)
//! - a `<Type>Filter` shape carrying every column listed in the
//!   List verb's `filterable_columns`
//!
//! Determinism: fields are emitted in the order the resource's `IndexMap`
//! exposes them after canonicalization (lexical), so two runs over the
//! same state file produce byte-identical output.

use crate::codegen::structs::naming;
use crate::codegen::structs::sql_map;
use crate::state::{
    FieldName, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState,
};
use std::collections::BTreeSet;

/// Render the full body of `<table>.rs` (without the codegen marker — the
/// caller prepends the marker). Returns owned `String`.
pub fn render_resource_body(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let mut out = String::new();

    out.push_str(&imports_block(table, resource));
    out.push('\n');

    let present_variants = collect_present_variants(resource);

    if present_variants.contains(&FieldVariant::Db) {
        out.push_str(&db_struct(table, resource));
        out.push('\n');
    }

    for variant in [
        FieldVariant::Insertable,
        FieldVariant::Patch,
        FieldVariant::Public,
        FieldVariant::Admin,
    ] {
        if !present_variants.contains(&variant) {
            continue;
        }
        out.push_str(&projection_struct(table, resource, variant));
        out.push('\n');
    }

    match list_options(resource) {
        Some(opts) if !opts.filterable_columns.is_empty() => {
            out.push_str(&filter_struct(table, resource, &opts.filterable_columns));
            out.push('\n');
        }
        _absent_or_empty => {}
    }

    out
}

fn imports_block(table: &str, resource: &ResourceState) -> String {
    let mut out = String::new();
    out.push_str("use serde::{Deserialize, Serialize};\n");

    let present = collect_present_variants(resource);
    let needs_diesel_table = present.contains(&FieldVariant::Db)
        || present.contains(&FieldVariant::Insertable)
        || present.contains(&FieldVariant::Patch);
    if needs_diesel_table {
        out.push_str(&format!(
            "use crate::database::schema::{table};\n",
            table = table,
        ));
    }

    let mut diesel_traits: Vec<&str> = Vec::new();
    if present.contains(&FieldVariant::Db) {
        diesel_traits.push("Queryable");
        diesel_traits.push("Selectable");
        diesel_traits.push("Identifiable");
    }
    if present.contains(&FieldVariant::Insertable) {
        diesel_traits.push("Insertable");
    }
    if present.contains(&FieldVariant::Patch) {
        diesel_traits.push("AsChangeset");
    }
    if !diesel_traits.is_empty() {
        diesel_traits.sort();
        diesel_traits.dedup();
        out.push_str(&format!(
            "use diesel::{{{}}};\n",
            diesel_traits.join(", "),
        ));
    }

    out
}

fn db_struct(table: &str, resource: &ResourceState) -> String {
    let type_name = naming::type_stem(table);
    let mut out = String::new();
    out.push_str("#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize)]\n");
    out.push_str(&format!("#[diesel(table_name = {table})]\n", table = table));
    out.push_str(&format!("pub struct {type_name} {{\n"));
    for (name, field) in fields_for_variant(resource, FieldVariant::Db) {
        let ty = sql_map::rust_type(&field.sql_type, field.nullable);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n");
    out
}

fn projection_struct(
    table: &str,
    resource: &ResourceState,
    variant: FieldVariant,
) -> String {
    let struct_name = naming::struct_name_for_variant(table, variant);
    let derives = derives_for_variant(variant);
    let table_attr = table_attr_for_variant(variant, table);

    let mut out = String::new();
    out.push_str(&format!("#[derive({derives})]\n"));
    match table_attr {
        Some(attr) => out.push_str(&format!("{attr}\n")),
        None => {}
    }
    out.push_str(&format!("pub struct {struct_name} {{\n"));
    for (name, field) in fields_for_variant(resource, variant) {
        let ty = field_type_for_variant(field, variant);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n");
    out
}

fn filter_struct(
    table: &str,
    resource: &ResourceState,
    filterable: &BTreeSet<FieldName>,
) -> String {
    let struct_name = naming::filter_struct_name(table);
    let mut out = String::new();
    out.push_str("#[derive(Debug, Default, Clone, Serialize, Deserialize)]\n");
    out.push_str(&format!("pub struct {struct_name} {{\n"));
    for (name, field) in resource.fields.iter() {
        if !filterable.contains(name) {
            continue;
        }
        let ty = sql_map::rust_type_always_optional(&field.sql_type);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n");
    out
}

fn derives_for_variant(variant: FieldVariant) -> &'static str {
    match variant {
        FieldVariant::Db => {
            "Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize"
        }
        FieldVariant::Insertable => "Debug, Clone, Insertable, Serialize, Deserialize",
        FieldVariant::Patch => "Debug, Default, Clone, AsChangeset, Serialize, Deserialize",
        FieldVariant::Public => "Debug, Clone, Serialize, Deserialize",
        FieldVariant::Admin => "Debug, Clone, Serialize, Deserialize",
    }
}

fn table_attr_for_variant(variant: FieldVariant, table: &str) -> Option<String> {
    match variant {
        FieldVariant::Db | FieldVariant::Insertable | FieldVariant::Patch => {
            Some(format!("#[diesel(table_name = {table})]"))
        }
        FieldVariant::Public | FieldVariant::Admin => None,
    }
}

fn field_type_for_variant(field: &FieldState, variant: FieldVariant) -> String {
    match variant {
        FieldVariant::Patch => sql_map::rust_type_always_optional(&field.sql_type),
        FieldVariant::Db
        | FieldVariant::Insertable
        | FieldVariant::Public
        | FieldVariant::Admin => sql_map::rust_type(&field.sql_type, field.nullable),
    }
}

fn fields_for_variant<'a>(
    resource: &'a ResourceState,
    variant: FieldVariant,
) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource
        .fields
        .iter()
        .filter(|(_, field)| field.variants.contains(&variant))
        .collect()
}

fn collect_present_variants(resource: &ResourceState) -> BTreeSet<FieldVariant> {
    let mut present: BTreeSet<FieldVariant> = BTreeSet::new();
    for field in resource.fields.values() {
        for v in &field.variants {
            present.insert(*v);
        }
    }
    present
}

fn list_options(resource: &ResourceState) -> Option<&ListOptions> {
    let state: &VerbState = resource.verbs.get(&Verb::List)?;
    state.list_options.as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::ResourceName;
    use crate::state::{
        AuthMode, FieldName, FieldState, FieldVariant, ListOptions, SqlType, Verb,
        VerbState,
    };
    use indexmap::IndexMap;
    use std::collections::BTreeSet;

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
        }
    }

    fn full_resource(table: &str) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            field(
                "Int8",
                &[
                    FieldVariant::Db,
                    FieldVariant::Public,
                    FieldVariant::Admin,
                ],
                false,
                true,
            ),
        );
        fields.insert(
            FieldName::new("email"),
            field(
                "Varchar",
                &[
                    FieldVariant::Db,
                    FieldVariant::Insertable,
                    FieldVariant::Patch,
                    FieldVariant::Public,
                    FieldVariant::Admin,
                ],
                false,
                false,
            ),
        );
        fields.insert(
            FieldName::new("password_hash"),
            field("Varchar", &[FieldVariant::Db], false, false),
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        let mut filterable: BTreeSet<FieldName> = BTreeSet::new();
        filterable.insert(FieldName::new("email"));
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
            },
        );

        let mut resource = ResourceState::new(ResourceName::new(table));
        resource.fields = fields;
        resource.verbs = verbs;
        resource.canonicalize();
        resource
    }

    #[test]
    fn emits_all_five_projections_when_all_variants_present() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);

        assert!(body.contains("pub struct User {"), "Db base struct missing:\n{body}");
        assert!(
            body.contains("pub struct UserInsertable {"),
            "Insertable missing:\n{body}",
        );
        assert!(
            body.contains("pub struct UserPatch {"),
            "Patch missing:\n{body}",
        );
        assert!(
            body.contains("pub struct UserPublic {"),
            "Public missing:\n{body}",
        );
        assert!(
            body.contains("pub struct UserAdmin {"),
            "Admin missing:\n{body}",
        );
    }

    #[test]
    fn patch_wraps_every_field_in_option() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);

        let patch_section = body.split("pub struct UserPatch {").nth(1).expect("patch start");
        let patch_section = patch_section.split('}').next().expect("patch end");

        assert!(
            patch_section.contains("pub email: Option<String>"),
            "non-nullable email should still be Option in Patch:\n{patch_section}",
        );
        for line in patch_section.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("pub ") {
                continue;
            }
            assert!(
                trimmed.contains("Option<"),
                "Patch field not wrapped in Option: {trimmed}",
            );
        }
    }

    #[test]
    fn insertable_excludes_non_insertable_fields() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        let ins_section = body
            .split("pub struct UserInsertable {")
            .nth(1)
            .expect("ins start");
        let ins_section = ins_section.split('}').next().expect("ins end");
        assert!(
            ins_section.contains("pub email: String"),
            "email present in Insertable",
        );
        assert!(
            !ins_section.contains("pub id:"),
            "id is not Insertable here, must be excluded",
        );
        assert!(
            !ins_section.contains("password_hash"),
            "password_hash is Db-only, must be excluded",
        );
    }

    #[test]
    fn public_excludes_db_only_fields() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        let pub_section = body.split("pub struct UserPublic {").nth(1).expect("pub start");
        let pub_section = pub_section.split('}').next().expect("pub end");
        assert!(
            !pub_section.contains("password_hash"),
            "password_hash leaked into Public",
        );
        assert!(pub_section.contains("pub id:"));
        assert!(pub_section.contains("pub email: String"));
    }

    #[test]
    fn filter_only_includes_filterable_columns() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        assert!(body.contains("pub struct UserFilter {"));
        let filter_section = body
            .split("pub struct UserFilter {")
            .nth(1)
            .expect("filter start");
        let filter_section = filter_section.split('}').next().expect("filter end");
        assert!(filter_section.contains("pub email: Option<String>"));
        assert!(
            !filter_section.contains("pub id:"),
            "id is not in filterable_columns; must be excluded",
        );
        assert!(
            !filter_section.contains("password_hash"),
            "password_hash never filterable",
        );
    }

    #[test]
    fn no_filter_struct_when_filterable_columns_empty() {
        let mut resource = full_resource("users");
        match resource.verbs.get_mut(&Verb::List) {
            Some(state) => match &mut state.list_options {
                Some(opts) => {
                    opts.filterable_columns.clear();
                }
                None => {}
            },
            None => {}
        }
        let body = render_resource_body(&resource);
        assert!(
            !body.contains("UserFilter"),
            "Filter struct should be omitted when no filterable columns",
        );
    }

    #[test]
    fn no_filter_struct_when_no_list_verb() {
        let mut resource = full_resource("users");
        resource.verbs.shift_remove(&Verb::List);
        let body = render_resource_body(&resource);
        assert!(!body.contains("UserFilter"));
    }

    #[test]
    fn db_struct_uses_diesel_table_attr() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        assert!(body.contains("#[diesel(table_name = users)]"));
    }

    #[test]
    fn missing_db_variant_omits_base_struct() {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("payload"),
            field("Jsonb", &[FieldVariant::Public], false, false),
        );
        let mut resource = ResourceState::new(ResourceName::new("events"));
        resource.fields = fields;
        resource.canonicalize();
        let body = render_resource_body(&resource);
        assert!(
            !body.contains("pub struct Event {"),
            "Db base struct should be absent when no field carries Db variant",
        );
        assert!(body.contains("pub struct EventPublic {"));
    }

    #[test]
    fn nullable_column_yields_option_in_db() {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            field("Int8", &[FieldVariant::Db, FieldVariant::Public], false, true),
        );
        fields.insert(
            FieldName::new("nickname"),
            field("Varchar", &[FieldVariant::Db, FieldVariant::Public], true, false),
        );
        let mut resource = ResourceState::new(ResourceName::new("users"));
        resource.fields = fields;
        resource.canonicalize();
        let body = render_resource_body(&resource);
        let db_section = body.split("pub struct User {").nth(1).expect("db start");
        let db_section = db_section.split('}').next().expect("db end");
        assert!(db_section.contains("pub nickname: Option<String>"));
        assert!(db_section.contains("pub id: i64"));
    }
}
