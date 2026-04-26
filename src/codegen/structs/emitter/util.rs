//! Shared helpers used across the emitter submodules.
//!
//! Kept to a single file so the cross-cutting concerns (variant filtering,
//! list-options lookup, derive lists, table attributes, type rendering)
//! live in one place rather than being duplicated per emitter.

use crate::codegen::structs::sql_map;
use crate::state::{
    FieldName, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState,
};
use std::collections::BTreeSet;

/// Walk the resource's fields and return every `FieldVariant` that
/// appears at least once. Drives the "should we emit this struct?"
/// branching in the runner.
pub fn collect_present_variants(resource: &ResourceState) -> BTreeSet<FieldVariant> {
    let mut present: BTreeSet<FieldVariant> = BTreeSet::new();
    for field in resource.fields.values() {
        for v in &field.variants {
            present.insert(*v);
        }
    }
    present
}

/// Borrow the `ListOptions` for the resource's `List` verb, if any.
pub fn list_options(resource: &ResourceState) -> Option<&ListOptions> {
    let state: &VerbState = resource.verbs.get(&Verb::List)?;
    state.list_options.as_ref()
}

/// Filter the resource's fields to those carrying a given variant, in
/// the canonical iteration order (lexical post-`canonicalize`).
pub fn fields_for_variant<'a>(
    resource: &'a ResourceState,
    variant: FieldVariant,
) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource
        .fields
        .iter()
        .filter(|(_, field)| field.variants.contains(&variant))
        .collect()
}

/// Returns `true` when every field in `variant` also carries the `Db`
/// variant — i.e. the projection is a column-by-column subset of the Db
/// row. Required for the `From<<DbType>>` impl to compile.
pub fn projection_is_db_subset(resource: &ResourceState, variant: FieldVariant) -> bool {
    for (_, field) in resource.fields.iter() {
        if !field.variants.contains(&variant) {
            continue;
        }
        if !field.variants.contains(&FieldVariant::Db) {
            return false;
        }
    }
    true
}

/// `#[derive(...)]` list per projection variant. The Db base struct
/// gets the full Diesel reading set; mutation variants pick up
/// `Insertable` / `AsChangeset`; `Public` / `Admin` are pure data.
pub fn derives_for_variant(variant: FieldVariant) -> &'static str {
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

/// Diesel `#[diesel(table_name = ...)]` attribute, when the variant is
/// one of the schema-bound ones. `Public` / `Admin` do not bind to a
/// table and therefore do not get the attribute.
pub fn table_attr_for_variant(variant: FieldVariant, table: &str) -> Option<String> {
    match variant {
        FieldVariant::Db | FieldVariant::Insertable | FieldVariant::Patch => {
            Some(format!("#[diesel(table_name = {table})]"))
        }
        FieldVariant::Public | FieldVariant::Admin => None,
    }
}

/// Render the Rust type for a field on a given variant. Patch always
/// wraps in `Option`; everything else honors the column's nullability.
pub fn field_type_for_variant(field: &FieldState, variant: FieldVariant) -> String {
    match variant {
        FieldVariant::Patch => sql_map::rust_type_always_optional(&field.sql_type),
        FieldVariant::Db
        | FieldVariant::Insertable
        | FieldVariant::Public
        | FieldVariant::Admin => sql_map::rust_type(&field.sql_type, field.nullable),
    }
}
