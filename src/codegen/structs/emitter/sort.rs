//! `<Type>Sort` enum emitter.
//!
//! Drives the List endpoint's `?sort=col` / `?sort=-col` query shape.
//! For every column listed in `sortable_columns`, the enum gets a pair
//! of variants `<ColPascal>Asc` and `<ColPascal>Desc`. A `Default` impl
//! picks the primary key column ascending (or the first sortable column
//! if no PK is present), and a `FromStr` impl parses the wire form.

use std::collections::BTreeSet;

use crate::{
    codegen::structs::naming,
    state::{FieldName, ResourceState},
};

pub fn render(resource: &ResourceState, sortable: &BTreeSet<FieldName>) -> String {
    let enum_name = naming::sort_enum_name_for_resource(resource);
    let table = resource.name.as_str();

    let mut variants: Vec<(String, String, String)> = Vec::new();
    for (name, _field) in resource.fields.iter() {
        if !sortable.contains(name) {
            continue;
        }
        let col = name.as_str();
        let pascal = naming::pascal_case(col);
        variants.push((col.to_string(), format!("{pascal}Asc"), format!("{pascal}Desc")));
    }

    let default_variant = default_sort_variant(resource, sortable, &variants);

    let mut out = String::new();
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]\n");
    out.push_str(&format!("pub enum {enum_name} {{\n"));
    for (_col, asc, desc) in &variants {
        out.push_str(&format!("    {asc},\n"));
        out.push_str(&format!("    {desc},\n"));
    }
    out.push_str("}\n\n");

    // Default impl
    out.push_str(&format!("impl Default for {enum_name} {{\n"));
    out.push_str("    fn default() -> Self {\n");
    out.push_str(&format!("        Self::{default_variant}\n"));
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // FromStr impl (?sort=col / ?sort=-col)
    out.push_str(&format!("impl FromStr for {enum_name} {{\n"));
    out.push_str("    type Err = String;\n");
    out.push_str("    fn from_str(s: &str) -> Result<Self, Self::Err> {\n");
    out.push_str("        let (col, desc) = match s.strip_prefix('-') {\n");
    out.push_str("            Some(rest) => (rest, true),\n");
    out.push_str("            None => (s, false),\n");
    out.push_str("        };\n");
    out.push_str("        match (col, desc) {\n");
    for (col, asc, desc) in &variants {
        out.push_str(&format!("            (\"{col}\", false) => Ok(Self::{asc}),\n", col = col, asc = asc,));
        out.push_str(&format!("            (\"{col}\", true) => Ok(Self::{desc}),\n", col = col, desc = desc,));
    }
    out.push_str(&format!("            _other => Err(format!(\"unknown sort column for {table}: {{}}\", s)),\n", table = table,));
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

/// Pick the default sort variant — primary key column (`<Pk>Asc`) when
/// it appears in `sortable_columns`, else the first sortable column in
/// emission order. Falls back to a synthesized `Unspecified` literal
/// only when `variants` is empty (shouldn't happen — the runner guards
/// emission on `sortable_columns` being non-empty — but defending
/// against a future refactor moving that guard).
fn default_sort_variant(resource: &ResourceState, sortable: &BTreeSet<FieldName>, variants: &[(String, String, String)]) -> String {
    for (name, field) in resource.fields.iter() {
        if !field.primary_key {
            continue;
        }
        if !sortable.contains(name) {
            continue;
        }
        let pascal = naming::pascal_case(name.as_str());
        return format!("{pascal}Asc");
    }
    match variants.first() {
        Some((_col, asc, _desc)) => asc.clone(),
        None => "Unspecified".to_string(),
    }
}
