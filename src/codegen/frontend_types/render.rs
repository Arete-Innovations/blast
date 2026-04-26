//! TypeScript interface rendering for per-resource types.
//!
//! Maps Diesel SQL types to TypeScript types using the same table as
//! `src/codegen/structs/sql_map.rs` (which maps SQL → Rust).
//!
//! Convention: snake_case field names, per Governor rule.

use crate::codegen::structs::naming::{
    filter_struct_name_for_resource, type_stem_for_resource,
};
use crate::state::{FieldState, FieldVariant, FilterKind, ResourceState, SqlType, Verb};

/// Map a Diesel SQL type to its TypeScript type.
/// Case-insensitive, same catalogue as `sql_map::rust_base_type`.
pub fn ts_base_type(sql: &SqlType) -> &'static str {
    let lowered = sql.as_str().to_ascii_lowercase();
    match lowered.as_str() {
        "bool" | "boolean" => "boolean",
        "int2" | "smallint" | "smallserial"
        | "int4" | "integer" | "serial"
        | "int8" | "bigint" | "bigserial"
        | "float4" | "real"
        | "float8" | "double" | "double precision"
        | "numeric" | "decimal" => "number",
        "text" | "varchar" | "bpchar" | "char" | "citext"
        | "uuid"
        | "bytea"
        | "timestamp" | "timestamptz"
        | "date" | "time" => "string",
        "json" | "jsonb" => "unknown",
        // Unknown types fall back to string (matches sql_map.rs fallback).
        _other => "string",
    }
}

fn ts_type(field: &FieldState) -> String {
    let base = ts_base_type(&field.sql_type);
    if field.nullable {
        format!("{base} | null")
    } else {
        base.to_string()
    }
}

fn ts_type_always_optional(field: &FieldState) -> String {
    let base = ts_base_type(&field.sql_type);
    format!("{base} | null")
}

fn filter_ts_type(sql: &SqlType, kind: FilterKind) -> String {
    let base = ts_base_type(sql);
    match kind {
        FilterKind::Eq => format!("{base} | null"),
        FilterKind::Range => format!("{{ from: {base} | null; to: {base} | null }} | null"),
        FilterKind::IlikeContains => "string | null".to_string(),
        FilterKind::In => format!("{base}[] | null"),
        FilterKind::Bool => "boolean | null".to_string(),
    }
}

/// Build the full TS file body for one resource.
pub fn build_resource_types(resource: &ResourceState) -> String {
    let stem = type_stem_for_resource(resource);

    let mut out = String::new();

    // --- Db struct (only if Db fields exist) ---
    let db_fields: Vec<_> = resource
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Db))
        .collect();
    if !db_fields.is_empty() {
        out.push_str(&format!("export interface {stem} {{\n"));
        for (name, field) in &db_fields {
            let ty = ts_type(field);
            out.push_str(&format!("  {}: {ty}\n", name.as_str()));
        }
        out.push_str("}\n\n");
    }

    // --- Insertable ---
    let insertable_fields: Vec<_> = resource
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Insertable))
        .collect();
    if !insertable_fields.is_empty() {
        out.push_str(&format!("export interface {stem}Insertable {{\n"));
        for (name, field) in &insertable_fields {
            let ty = ts_type(field);
            out.push_str(&format!("  {}: {ty}\n", name.as_str()));
        }
        out.push_str("}\n\n");
    }

    // --- Patch (all fields optional) ---
    let patch_fields: Vec<_> = resource
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Patch))
        .collect();
    if !patch_fields.is_empty() {
        out.push_str(&format!("export interface {stem}Patch {{\n"));
        for (name, field) in &patch_fields {
            let ty = ts_type_always_optional(field);
            out.push_str(&format!("  {}?: {ty}\n", name.as_str()));
        }
        out.push_str("}\n\n");
    }

    // --- Public ---
    let public_fields: Vec<_> = resource
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Public))
        .collect();
    if !public_fields.is_empty() {
        out.push_str(&format!("export interface {stem}Public {{\n"));
        for (name, field) in &public_fields {
            let ty = ts_type(field);
            out.push_str(&format!("  {}: {ty}\n", name.as_str()));
        }
        out.push_str("}\n\n");
    }

    // --- Admin ---
    let admin_fields: Vec<_> = resource
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Admin))
        .collect();
    if !admin_fields.is_empty() {
        out.push_str(&format!("export interface {stem}Admin {{\n"));
        for (name, field) in &admin_fields {
            let ty = ts_type(field);
            out.push_str(&format!("  {}: {ty}\n", name.as_str()));
        }
        out.push_str("}\n\n");
    }

    emit_filter_interface(resource, &mut out);

    let trimmed = out.trim_end_matches('\n');
    format!("{trimmed}\n")
}

fn emit_filter_interface(resource: &ResourceState, out: &mut String) {
    let list_verb = match resource.verbs.get(&Verb::List) {
        Some(v) => v,
        None => return,
    };
    let list_opts = match list_verb.list_options.as_ref() {
        Some(opts) => opts,
        None => return,
    };
    let filterable_cols = &list_opts.filterable_columns;
    if filterable_cols.is_empty() {
        return;
    }
    let filter_name = filter_struct_name_for_resource(resource);
    out.push_str(&format!("export interface {filter_name} {{\n"));
    for (name, field) in resource.fields.iter() {
        let kind = match filterable_cols.get(name) {
            Some(k) => *k,
            None => continue, // allow: field not in filterable_cols; skip it
        };
        let ty = filter_ts_type(&field.sql_type, kind);
        out.push_str(&format!("  {}?: {ty}\n", name.as_str()));
    }
    out.push_str("}\n\n");
}

pub fn meltdown_ts() -> &'static str {
    "export interface MeltDownError {\n\
  code: number\n\
  type: string\n\
  message: string\n\
  context: Record<string, string> | null\n\
}\n\
\n\
export interface MeltDownResponse {\n\
  error: MeltDownError\n\
}\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::{FieldName, ResourceName};
    use crate::state::resource::{
        AuthMode, FieldState, FieldVariant, FilterKind, ListOptions, ResourceState, Verb,
        VerbState, RESOURCE_SCHEMA_VERSION,
    };
    use crate::state::SqlType;
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let all_v: BTreeSet<FieldVariant> = [
            FieldVariant::Db,
            FieldVariant::Insertable,
            FieldVariant::Patch,
            FieldVariant::Public,
        ]
        .into_iter()
        .collect();
        let id_v: BTreeSet<FieldVariant> =
            [FieldVariant::Db, FieldVariant::Public].into_iter().collect();

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: id_v,
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("email"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
        filterable.insert(FieldName::new("email"), FilterKind::IlikeContains);

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: filterable,
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: None,
                }),
            },
        );
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("users"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
        }
    }

    #[test]
    fn emits_public_interface() {
        let r = synth_resource();
        let body = build_resource_types(&r);
        assert!(body.contains("export interface UserPublic {"), "UserPublic missing");
        assert!(body.contains("  id: number"), "id field missing");
        assert!(body.contains("  email: string"), "email field missing");
    }

    #[test]
    fn emits_insertable_interface() {
        let r = synth_resource();
        let body = build_resource_types(&r);
        assert!(body.contains("export interface UserInsertable {"), "UserInsertable missing");
        assert!(
            !body.contains("  id: number\nexport interface UserInsertable"),
            "id should not be in insertable"
        );
    }

    #[test]
    fn emits_patch_with_optional_fields() {
        let r = synth_resource();
        let body = build_resource_types(&r);
        assert!(body.contains("export interface UserPatch {"), "UserPatch missing");
        assert!(body.contains("  email?: string | null"), "email in patch must be optional");
    }

    #[test]
    fn emits_filter_interface_when_list_verb_present() {
        let r = synth_resource();
        let body = build_resource_types(&r);
        assert!(body.contains("export interface UserFilter {"), "UserFilter missing");
        assert!(body.contains("  email?: string | null"), "filter email must be optional string");
    }

    #[test]
    fn ts_base_type_maps_common_types() {
        assert_eq!(ts_base_type(&SqlType::new("Int8")), "number");
        assert_eq!(ts_base_type(&SqlType::new("Varchar")), "string");
        assert_eq!(ts_base_type(&SqlType::new("Bool")), "boolean");
        assert_eq!(ts_base_type(&SqlType::new("Timestamptz")), "string");
        assert_eq!(ts_base_type(&SqlType::new("Jsonb")), "unknown");
        assert_eq!(ts_base_type(&SqlType::new("Uuid")), "string");
        assert_eq!(ts_base_type(&SqlType::new("unknown_type")), "string");
    }

    #[test]
    fn meltdown_ts_exports_correct_shape() {
        let body = meltdown_ts();
        assert!(body.contains("export interface MeltDownResponse"));
        assert!(body.contains("export interface MeltDownError"));
        assert!(body.contains("code: number"));
        assert!(body.contains("type: string"));
        assert!(body.contains("message: string"));
        assert!(body.contains("context: Record<string, string> | null"));
    }
}
