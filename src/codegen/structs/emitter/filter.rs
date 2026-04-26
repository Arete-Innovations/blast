//! `<Type>Filter` struct emitter (v2 shape).
//!
//! Drives the List endpoint's `?filter[col]=val` query shape. The state
//! declares `filterable_columns` as a set of column names; this module
//! turns that into a struct where every selected column is wrapped in
//! `Option<<col_type>>`.
//!
//! v3 will type each field by its declared `FilterKind` once the
//! state-extensions branch lands.

use crate::codegen::structs::naming;
use crate::codegen::structs::sql_map;
use crate::state::{FieldName, ResourceState};
use std::collections::BTreeSet;

pub fn render(resource: &ResourceState, filterable: &BTreeSet<FieldName>) -> String {
    let struct_name = naming::filter_struct_name_for_resource(resource);
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
