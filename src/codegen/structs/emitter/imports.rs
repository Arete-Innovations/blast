//! Imports header for a per-resource generated file.
//!
//! Every emitted file opens with the `use` lines the rest of the body
//! needs — `serde`, the diesel table module, the diesel trait set.
//! Import lines are tailored per-resource so the generated file never
//! carries dead `use`s that would trigger `unused_imports` warnings in
//! the user app.

use super::util;
use crate::state::{FieldVariant, GenLevel, ResourceState};

pub fn render(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let mut out = String::new();
    out.push_str("use serde::{Deserialize, Serialize};\n");

    let present = util::collect_present_variants(resource);
    let needs_diesel_table = present.contains(&FieldVariant::Db) || present.contains(&FieldVariant::Insertable) || present.contains(&FieldVariant::Patch);
    if needs_diesel_table {
        out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
        out.push_str(&format!("use crate::database::schema::{table};\n", table = table,));
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
        out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
        out.push_str(&format!("use diesel::{{{}}};\n", diesel_traits.join(", "),));
    }

    // leptos-struct-table TableRow derive expansion references its own
    // crate's TableDataProvider trait at the call site; the derive macro
    // emits `impl TableDataProvider for Vec<Self>` when `impl_vec_data_provider`
    // is set, which needs the trait in scope. Star-glob the crate so the
    // generated file is self-sufficient.
    let needs_lst = resource.gen_level >= GenLevel::Components && present.contains(&FieldVariant::Public);
    if needs_lst {
        out.push_str("use leptos_struct_table::*;\n");
    }

    // Sort enum needs FromStr; only emit the use when we actually emit
    // a sort enum to keep unused-import warnings out of the user app.
    // The `None` arm here is not an error: a resource without a List
    // verb has no sort enum, full stop — no caller misled.
    let sort_present = match util::list_options(resource) {
        Some(opts) => !opts.sortable_columns.is_empty(),
        None => false, // allow: absence of List verb means no sort enum, by spec
    };
    if sort_present {
        out.push_str("use std::str::FromStr;\n");
    }

    out
}
