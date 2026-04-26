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
//! 4. `From<<Type>>` impls for `Public` / `Admin` (when projection is a
//!    Db subset and the Db base struct itself is present)
//! 5. `<Type>Filter` (when `filterable_columns` non-empty)
//! 6. `<Type>Sort` (when `sortable_columns` non-empty)

use super::{db, filter, from_impl, imports, projection, sort, util};
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

    // From<Type> impls for projections that are subsets of the Db row.
    // Only emit when the Db base struct itself is present — without it
    // there is no source type to convert from.
    if present_variants.contains(&FieldVariant::Db) {
        for variant in [FieldVariant::Public, FieldVariant::Admin] {
            if !present_variants.contains(&variant) {
                continue;
            }
            if !util::projection_is_db_subset(resource, variant) {
                continue;
            }
            out.push_str(&from_impl::render(resource, variant));
            out.push('\n');
        }
    }

    match util::list_options(resource) {
        Some(opts) if !opts.filterable_columns.is_empty() => {
            out.push_str(&filter::render(resource, &opts.filterable_columns));
            out.push('\n');
        }
        _absent_or_empty => {}
    }

    match util::list_options(resource) {
        Some(opts) if !opts.sortable_columns.is_empty() => {
            out.push_str(&sort::render(resource, &opts.sortable_columns));
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
        AuthMode, FieldName, FieldState, FieldVariant, FilterKind, ListOptions, SqlType,
        Verb, VerbState,
    };
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

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
        let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
        filterable.insert(FieldName::new("email"), FilterKind::IlikeContains);
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
    fn from_db_impl_emitted_for_public_and_admin() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        assert!(
            body.contains("impl From<User> for UserPublic"),
            "missing From<User> for UserPublic:\n{body}",
        );
        assert!(
            body.contains("impl From<User> for UserAdmin"),
            "missing From<User> for UserAdmin:\n{body}",
        );
    }

    #[test]
    fn from_db_impl_moves_each_subset_field() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        let pub_impl = body
            .split("impl From<User> for UserPublic")
            .nth(1)
            .expect("pub impl start")
            .split("}\n}")
            .next()
            .expect("pub impl end");
        assert!(pub_impl.contains("id: row.id"), "missing id move:\n{pub_impl}");
        assert!(
            pub_impl.contains("email: row.email"),
            "missing email move:\n{pub_impl}",
        );
        assert!(
            !pub_impl.contains("password_hash"),
            "password_hash must not appear in From<User> for UserPublic",
        );
    }

    #[test]
    fn no_from_impl_when_db_variant_absent() {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("payload"),
            field("Jsonb", &[FieldVariant::Public], false, false),
        );
        let mut resource = ResourceState::new(ResourceName::new("events"));
        resource.fields = fields;
        resource.canonicalize();
        let body = render_resource_body(&resource);
        assert!(!body.contains("impl From<"), "no Db source -> no From impl:\n{body}");
    }

    #[test]
    fn no_from_impl_when_projection_not_db_subset() {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            field("Int8", &[FieldVariant::Db, FieldVariant::Public], false, true),
        );
        fields.insert(
            FieldName::new("display_name"),
            field("Varchar", &[FieldVariant::Public], false, false),
        );
        let mut resource = ResourceState::new(ResourceName::new("users"));
        resource.fields = fields;
        resource.canonicalize();
        let body = render_resource_body(&resource);
        assert!(
            !body.contains("impl From<User> for UserPublic"),
            "Public is not a Db subset; From impl must be omitted:\n{body}",
        );
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
    fn filter_field_types_per_kind() {
        let mut resource = full_resource("users");
        resource.fields.insert(
            FieldName::new("created_at"),
            field("Timestamptz", &[FieldVariant::Db, FieldVariant::Public], false, false),
        );
        resource.fields.insert(
            FieldName::new("age"),
            field("Int4", &[FieldVariant::Db, FieldVariant::Public], false, false),
        );
        resource.fields.insert(
            FieldName::new("status"),
            field("Varchar", &[FieldVariant::Db, FieldVariant::Public], false, false),
        );
        resource.fields.insert(
            FieldName::new("active"),
            field("Bool", &[FieldVariant::Db, FieldVariant::Public], false, false),
        );

        let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
        filterable.insert(FieldName::new("email"), FilterKind::IlikeContains);
        filterable.insert(FieldName::new("created_at"), FilterKind::Range);
        filterable.insert(FieldName::new("age"), FilterKind::Eq);
        filterable.insert(FieldName::new("status"), FilterKind::In);
        filterable.insert(FieldName::new("active"), FilterKind::Bool);

        match resource.verbs.get_mut(&Verb::List) {
            Some(state) => match state.list_options.as_mut() {
                Some(opts) => opts.filterable_columns = filterable,
                None => {}
            },
            None => {}
        }
        resource.canonicalize();
        let body = render_resource_body(&resource);

        let filter_section = body
            .split("pub struct UserFilter {")
            .nth(1)
            .expect("filter start");
        let filter_section = filter_section.split("}\n").next().expect("filter end");

        assert!(filter_section.contains("pub email: Option<String>"));
        assert!(filter_section.contains(
            "pub created_at: Option<RangeFilter<chrono::DateTime<chrono::Utc>>>"
        ));
        assert!(filter_section.contains("pub age: Option<i32>"));
        assert!(filter_section.contains("pub status: Option<Vec<String>>"));
        assert!(filter_section.contains("pub active: Option<bool>"));

        assert!(body.contains("pub struct RangeFilter<T>"));
        assert!(body.contains("pub from: Option<T>"));
        assert!(body.contains("pub to: Option<T>"));
    }

    #[test]
    fn no_range_filter_struct_when_no_range_kind() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        assert!(
            !body.contains("pub struct RangeFilter"),
            "RangeFilter only when a column uses FilterKind::Range",
        );
    }

    #[test]
    fn filter_derives_default_deserialize_debug_clone() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        let head = body.split("pub struct UserFilter").next().expect("head");
        let derive_line = head
            .lines()
            .rev()
            .find(|l| l.contains("#[derive"))
            .expect("derive line for UserFilter");
        assert!(derive_line.contains("Debug"));
        assert!(derive_line.contains("Default"));
        assert!(derive_line.contains("Clone"));
        assert!(derive_line.contains("Deserialize"));
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
        assert!(body.contains("impl From<Datum> for DatumPublic"));
        assert!(body.contains("impl From<Datum> for DatumAdmin"));
    }

    fn sortable_resource() -> ResourceState {
        let mut resource = full_resource("users");
        resource.fields.insert(
            FieldName::new("created_at"),
            field(
                "Timestamptz",
                &[FieldVariant::Db, FieldVariant::Public],
                false,
                false,
            ),
        );

        let mut sortable: BTreeSet<FieldName> = BTreeSet::new();
        sortable.insert(FieldName::new("id"));
        sortable.insert(FieldName::new("created_at"));

        match resource.verbs.get_mut(&Verb::List) {
            Some(state) => match state.list_options.as_mut() {
                Some(opts) => opts.sortable_columns = sortable,
                None => {}
            },
            None => {}
        }
        resource.canonicalize();
        resource
    }

    #[test]
    fn sort_enum_emitted_with_asc_desc_per_column() {
        let resource = sortable_resource();
        let body = render_resource_body(&resource);
        assert!(body.contains("pub enum UserSort {"));
        let sort_section = body.split("pub enum UserSort {").nth(1).expect("sort start");
        let sort_section = sort_section.split('}').next().expect("sort end");
        assert!(sort_section.contains("IdAsc"));
        assert!(sort_section.contains("IdDesc"));
        assert!(sort_section.contains("CreatedAtAsc"));
        assert!(sort_section.contains("CreatedAtDesc"));
    }

    #[test]
    fn sort_enum_default_picks_pk_asc() {
        let resource = sortable_resource();
        let body = render_resource_body(&resource);
        assert!(body.contains("Self::IdAsc"));
    }

    #[test]
    fn sort_enum_emits_fromstr_for_signed_prefix() {
        let resource = sortable_resource();
        let body = render_resource_body(&resource);
        assert!(body.contains("impl FromStr for UserSort"));
        assert!(body.contains("(\"id\", false) => Ok(Self::IdAsc)"));
        assert!(body.contains("(\"id\", true) => Ok(Self::IdDesc)"));
        assert!(body.contains("(\"created_at\", true) => Ok(Self::CreatedAtDesc)"));
    }

    #[test]
    fn no_sort_enum_when_sortable_columns_empty() {
        let resource = full_resource("users");
        let body = render_resource_body(&resource);
        assert!(!body.contains("pub enum UserSort"));
    }

    #[test]
    fn no_sort_enum_when_no_list_verb() {
        let mut resource = sortable_resource();
        resource.verbs.shift_remove(&Verb::List);
        let body = render_resource_body(&resource);
        assert!(!body.contains("pub enum UserSort"));
    }

    #[test]
    fn fromstr_use_only_emitted_when_sort_enum_emitted() {
        let plain = full_resource("users");
        let plain_body = render_resource_body(&plain);
        assert!(!plain_body.contains("use std::str::FromStr"));

        let with_sort = sortable_resource();
        let sort_body = render_resource_body(&with_sort);
        assert!(sort_body.contains("use std::str::FromStr"));
    }
}
