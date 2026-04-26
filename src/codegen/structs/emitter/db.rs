//! Db base struct emitter.
//!
//! Renders the unsuffixed `<Type>` row that backs Diesel `Queryable` /
//! `Selectable` / `Identifiable`. Exists when at least one field carries
//! `FieldVariant::Db`.

use super::util;
use crate::codegen::structs::naming;
use crate::codegen::structs::sql_map;
use crate::state::{FieldVariant, ResourceState};

pub fn render(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let type_name = naming::type_stem_for_resource(resource);
    let mut out = String::new();
    out.push_str("#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize)]\n");
    out.push_str(&format!("#[diesel(table_name = {table})]\n", table = table));
    out.push_str(&format!("pub struct {type_name} {{\n"));
    for (name, field) in util::fields_for_variant(resource, FieldVariant::Db) {
        let ty = sql_map::rust_type(&field.sql_type, field.nullable);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n");
    out
}
