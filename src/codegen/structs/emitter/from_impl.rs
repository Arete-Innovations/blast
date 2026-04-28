//! `From<<Type>> for <Type><Variant>` impl emitter.
//!
//! Emitted for projections that are strict subsets of the Db row
//! (`Public`, `Admin`). Saves the user app from writing the field-by-
//! field move by hand. Excluded for `Insertable` / `Patch`: those belong
//! to a different mutation flow and may legitimately carry fields that
//! are not present on the Db row (e.g. plaintext password on Insertable).

use super::util;
use crate::{
    codegen::structs::naming,
    state::{FieldVariant, ResourceState},
};

pub fn render(resource: &ResourceState, variant: FieldVariant) -> String {
    let from_name = naming::type_stem_for_resource(resource);
    let into_name = naming::struct_name_for_variant_resource(resource, variant);
    let bind = "row";
    let mut out = String::new();
    out.push_str(&format!("impl From<{from_name}> for {into_name} {{\n", from_name = from_name, into_name = into_name,));
    out.push_str(&format!("    fn from({bind}: {from_name}) -> Self {{\n", bind = bind, from_name = from_name,));
    out.push_str("        Self {\n");
    for (name, _field) in util::fields_for_variant(resource, variant) {
        let n = name.as_str();
        out.push_str(&format!("            {n}: {bind}.{n},\n", n = n, bind = bind,));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}
