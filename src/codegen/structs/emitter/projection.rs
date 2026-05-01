//! Projection-struct emitter — `Insertable`, `Patch`, `Public`, `Admin`.
//!
//! Each variant gets a struct named `<Type><Suffix>` carrying only the
//! fields whose state declares them as part of that variant. Diesel
//! attributes are stamped on the schema-bound variants only.

use super::util;
use crate::{
    codegen::structs::naming,
    state::{FieldVariant, ResourceState},
};

pub fn render(resource: &ResourceState, variant: FieldVariant) -> String {
    let table = resource.name.as_str();
    let struct_name = naming::struct_name_for_variant_resource(resource, variant);
    let derives = util::derives_for_variant(variant);
    let diesel_derives = util::diesel_derives_for_variant(variant);
    let table_attr = util::table_attr_for_variant(variant, table);

    let mut out = String::new();
    out.push_str(&format!("#[derive({derives})]\n"));
    match diesel_derives {
        Some(extra) => out.push_str(&format!("#[cfg_attr(not(target_arch = \"wasm32\"), derive({extra}))]\n")),
        None => {}
    }
    match table_attr {
        Some(attr) => out.push_str(&format!("{attr}\n")),
        None => {}
    }
    out.push_str(&format!("pub struct {struct_name} {{\n"));
    for (name, field) in util::fields_for_variant(resource, variant) {
        let ty = util::field_type_for_variant(field, variant);
        out.push_str(&format!("    pub {name}: {ty},\n", name = name.as_str(), ty = ty));
    }
    out.push_str("}\n");
    out
}
