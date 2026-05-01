//! Db base struct emitter.
//!
//! Renders the unsuffixed `<Type>` row that backs Diesel `Queryable` /
//! `Selectable` / `Identifiable`. Exists when at least one field carries
//! `FieldVariant::Db`.

use super::util;
use crate::{
    codegen::structs::{naming, sql_map},
    state::{FieldVariant, ResourceState},
};

pub fn render(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let type_name = naming::type_stem_for_resource(resource);
    let derives = util::derives_for_variant(FieldVariant::Db);
    let diesel_derives = util::diesel_derives_for_variant(FieldVariant::Db);
    let table_attr = util::table_attr_for_variant(FieldVariant::Db, table);

    let mut out = String::new();
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str(&format!("#[derive({derives})]\n"));
    match diesel_derives {
        Some(extra) => {
            out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
            out.push_str(&format!("#[derive({extra})]\n"));
        }
        None => {}
    }
    match table_attr {
        Some(attr) => out.push_str(&format!("{attr}\n")),
        None => {}
    }
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str(&format!("pub struct {type_name} {{\n"));
    for (name, field) in util::fields_for_variant(resource, FieldVariant::Db) {
        let ty = sql_map::rust_type(&field.sql_type, field.nullable);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n");
    out
}
