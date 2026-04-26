//! Imports header for a per-resource generated file.
//!
//! Every emitted file opens with the `use` lines the rest of the body
//! needs — `serde`, the diesel table module, the diesel trait set.
//! Import lines are tailored per-resource so the generated file never
//! carries dead `use`s that would trigger `unused_imports` warnings in
//! the user app.

use super::util;
use crate::state::{FieldVariant, ResourceState};

pub fn render(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let mut out = String::new();
    out.push_str("use serde::{Deserialize, Serialize};\n");

    let present = util::collect_present_variants(resource);
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
