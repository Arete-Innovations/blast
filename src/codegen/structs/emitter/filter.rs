//! `<Type>Filter` struct emitter.
//!
//! Drives the List endpoint's `?filter[col]=val` query shape. The state
//! declares `filterable_columns` as a map of column to `FilterKind`,
//! and this module turns that into a typed struct where each field is
//! wrapped in `Option<...>` and shaped by its `FilterKind`.

use crate::codegen::structs::naming;
use crate::codegen::structs::sql_map;
use crate::state::{FieldName, FilterKind, ResourceState, SqlType};
use std::collections::BTreeMap;

pub fn render(
    resource: &ResourceState,
    filterable: &BTreeMap<FieldName, FilterKind>,
) -> String {
    let struct_name = naming::filter_struct_name_for_resource(resource);
    let mut out = String::new();
    let needs_range = filterable
        .values()
        .any(|kind| matches!(kind, FilterKind::Range));
    out.push_str("#[derive(Debug, Default, Clone, Deserialize)]\n");
    out.push_str(&format!("pub struct {struct_name} {{\n"));
    for (name, field) in resource.fields.iter() {
        let kind = match filterable.get(name) {
            Some(k) => k,
            None => continue,
        };
        let ty = filter_field_type(&field.sql_type, *kind);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n");

    if needs_range {
        out.push('\n');
        out.push_str(&range_filter_struct());
    }
    out
}

/// `RangeFilter<T>` carries `from` + `to`, both `Option<T>`. Emitted
/// once per resource alongside the Filter struct when any filterable
/// column uses `FilterKind::Range`. Kept inline in the generated file
/// (rather than centralized in catalyst) because the type parameter
/// crosses the codegen / framework boundary and the duplication cost is
/// trivial vs. an extra import line in every consumer.
fn range_filter_struct() -> String {
    let mut out = String::new();
    out.push_str("#[derive(Debug, Default, Clone, Deserialize)]\n");
    out.push_str("pub struct RangeFilter<T> {\n");
    out.push_str("    pub from: Option<T>,\n");
    out.push_str("    pub to: Option<T>,\n");
    out.push_str("}\n");
    out
}

fn filter_field_type(sql: &SqlType, kind: FilterKind) -> String {
    let base = sql_map::rust_base_type(sql);
    match kind {
        FilterKind::Eq => format!("Option<{base}>"),
        FilterKind::Range => format!("Option<RangeFilter<{base}>>"),
        FilterKind::IlikeContains => "Option<String>".to_string(),
        FilterKind::In => format!("Option<Vec<{base}>>"),
        FilterKind::Bool => "Option<bool>".to_string(),
    }
}
