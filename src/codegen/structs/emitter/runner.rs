//! Top-level orchestration for the per-resource emitter.
//!
//! `render_resource_body` is the single public entry point: in,
//! `&ResourceState`; out, the file body (without the codegen marker —
//! the file-level runner in the parent module prepends the marker).
//! Each section emitter lives in its own sibling module; this file's
//! job is to call them in the right order with the right guards.
//!
//! Section order (deterministic):
//! 1. `use` imports
//! 2. base `<Type>` (when any field has `Db` variant)
//! 3. `<Type>{Insertable, Patch, Public, Admin}` (when present)
//! 4. `<Type>Filter` (when `filterable_columns` non-empty)

use super::{db, filter, imports, projection, util};
use crate::state::{FieldVariant, ResourceState};

pub fn render_resource_body(resource: &ResourceState) -> String {
    let mut out = String::new();

    out.push_str(&imports::render(resource));
    out.push('\n');

    let present_variants = util::collect_present_variants(resource);

    if present_variants.contains(&FieldVariant::Db) {
        out.push_str(&db::render(resource));
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
        out.push_str(&projection::render(resource, variant));
        out.push('\n');
    }

    match util::list_options(resource) {
        Some(opts) if !opts.filterable_columns.is_empty() => {
            out.push_str(&filter::render(resource, &opts.filterable_columns));
            out.push('\n');
        }
        _absent_or_empty => {}
    }

    out
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
        assert!(body.contains("pub struct UserInsertable {"));
        assert!(body.contains("pub struct UserPatch {"));
        assert!(body.contains("pub struct UserPublic {"));
        assert!(body.contains("pub struct UserAdmin {"));
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
        assert!(ins_section.contains("pub email: String"));
        assert!(!ins_section.contains("pub id:"));
        assert!(!ins_section.contains("password_hash"));
    }

    #[test]
    fn public_excludes_db_only_fields() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        let pub_section = body.split("pub struct UserPublic {").nth(1).expect("pub start");
        let pub_section = pub_section.split('}').next().expect("pub end");
        assert!(!pub_section.contains("password_hash"));
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
        assert!(!filter_section.contains("pub id:"));
        assert!(!filter_section.contains("password_hash"));
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
        assert!(!body.contains("UserFilter"));
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
        assert!(!body.contains("pub struct Event {"));
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

    #[test]
    fn singular_override_drives_struct_names() {
        let mut resource = full_resource("data");
        resource.singular_override = Some("datum".to_string());
        let body = render_resource_body(&resource);
        assert!(body.contains("pub struct Datum {"));
        assert!(body.contains("pub struct DatumPublic {"));
        assert!(body.contains("pub struct DatumAdmin {"));
        assert!(body.contains("pub struct DatumInsertable {"));
        assert!(body.contains("pub struct DatumPatch {"));
        assert!(body.contains("pub struct DatumFilter {"));
    }
}
