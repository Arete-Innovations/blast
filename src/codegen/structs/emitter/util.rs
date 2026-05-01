//! Shared helpers used across the emitter submodules.
//!
//! Kept to a single file so the cross-cutting concerns (variant filtering,
//! list-options lookup, derive lists, table attributes, type rendering)
//! live in one place rather than being duplicated per emitter.

use std::collections::BTreeSet;

use crate::{
    codegen::structs::sql_map,
    state::{FieldName, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState},
};

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
pub fn fields_for_variant<'a>(resource: &'a ResourceState, variant: FieldVariant) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource.fields.iter().filter(|(_, field)| field.variants.contains(&variant)).collect()
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

/// Wasm-safe `#[derive(...)]` list per projection variant. Diesel
/// derives (`Queryable` / `Selectable` / `Identifiable` / `Insertable` /
/// `AsChangeset`) must be gated to non-wasm via `cfg_attr` because
/// `diesel` is not available on the `wasm32-unknown-unknown` target.
pub fn derives_for_variant(variant: FieldVariant) -> &'static str {
    match variant {
        FieldVariant::Db | FieldVariant::Insertable | FieldVariant::Public | FieldVariant::Admin => "Debug, Clone, Serialize, Deserialize",
        FieldVariant::Patch => "Debug, Default, Clone, Serialize, Deserialize",
    }
}

/// Diesel-only derives that must be gated behind `cfg_attr(not(target_arch = "wasm32"), ...)`.
/// Returns `None` for variants that have no Diesel derives.
pub fn diesel_derives_for_variant(variant: FieldVariant) -> Option<&'static str> {
    match variant {
        FieldVariant::Db => Some("Queryable, Selectable, Identifiable"),
        FieldVariant::Insertable => Some("Insertable"),
        FieldVariant::Patch => Some("AsChangeset"),
        FieldVariant::Public | FieldVariant::Admin => None,
    }
}

/// Diesel `#[diesel(table_name = ...)]` attribute, when the variant is
/// one of the schema-bound ones. `Public` / `Admin` do not bind to a
/// table and therefore do not get the attribute. For Insertable / Patch
/// the attribute is emitted as a `cfg_attr` so it is suppressed on the
/// wasm target (those structs remain cross-target visible). For the Db
/// row the entire struct is wasm-gated, so a plain `#[diesel(...)]` is
/// sufficient.
pub fn table_attr_for_variant(variant: FieldVariant, table: &str) -> Option<String> {
    match variant {
        FieldVariant::Db => Some(format!("#[diesel(table_name = {table})]")),
        FieldVariant::Insertable | FieldVariant::Patch => {
            Some(format!("#[cfg_attr(not(target_arch = \"wasm32\"), diesel(table_name = {table}))]"))
        }
        FieldVariant::Public | FieldVariant::Admin => None,
    }
}

/// Render the Rust type for a field on a given variant. Patch always
/// wraps in `Option`; everything else honors the column's nullability.
pub fn field_type_for_variant(field: &FieldState, variant: FieldVariant) -> String {
    match variant {
        FieldVariant::Patch => sql_map::rust_type_always_optional(&field.sql_type),
        FieldVariant::Db | FieldVariant::Insertable | FieldVariant::Public | FieldVariant::Admin => sql_map::rust_type(&field.sql_type, field.nullable),
    }
}
