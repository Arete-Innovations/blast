//! TypeScript interface rendering for per-resource types.
//!
//! Maps Diesel SQL types to TypeScript types using the same table as
//! `src/codegen/structs/sql_map.rs` (which maps SQL → Rust).
//!
//! Convention: snake_case field names, per Governor rule.

use crate::codegen::components::input_map::{enum_meta, enum_options_const_name, enum_type_alias};
use crate::codegen::enums::ParsedEnum;
use crate::codegen::structs::naming::{filter_struct_name_for_resource, type_stem_for_resource};
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

fn ts_field_type(sql: &SqlType, enums: &[ParsedEnum]) -> String {
    match enum_meta(sql, enums) {
        Some((name, _variants)) => enum_type_alias(&name),
        None => ts_base_type(sql).to_string(), // allow: fall through to scalar SQL mapping
    }
}

fn ts_type(field: &FieldState, enums: &[ParsedEnum]) -> String {
    let base = ts_field_type(&field.sql_type, enums);
    if field.nullable {
        format!("{base} | null")
    } else {
        base
    }
}

fn ts_type_always_optional(field: &FieldState, enums: &[ParsedEnum]) -> String {
    let base = ts_field_type(&field.sql_type, enums);
    format!("{base} | null")
}

fn filter_ts_type(sql: &SqlType, kind: FilterKind, enums: &[ParsedEnum]) -> String {
    let base = ts_field_type(sql, enums);
    match kind {
        FilterKind::Eq => format!("{base} | null"),
        FilterKind::Range => format!("{{ from: {base} | null; to: {base} | null }} | null"),
        FilterKind::IlikeContains => "string | null".to_string(),
        FilterKind::In => format!("{base}[] | null"),
        FilterKind::Bool => "boolean | null".to_string(),
    }
}

/// Collect every distinct enum referenced by this resource's fields, in
/// the order they appear. Used by the runner to emit per-enum files and
/// by the renderer to emit `import type` lines for the resource module.
pub fn collect_resource_enums(resource: &ResourceState, enums: &[ParsedEnum]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (_, field) in resource.fields.iter() {
        match enum_meta(&field.sql_type, enums) {
            Some((name, variants)) => {
                if !out.iter().any(|(n, _)| n == &name) {
                    out.push((name, variants));
                }
            }
            None => continue, // allow: scalar SQL types contribute no enums
        }
    }
    out
}

/// Build the body of a per-enum TS module.
///
/// Emits the named string-literal-union alias plus a `readonly` values
/// constant the form codegen feeds to `<Dropdown :options="...">`.
pub fn build_enum_module(name: &str, variants: &[String]) -> String {
    let alias = enum_type_alias(name);
    let const_name = enum_options_const_name(name);
    let union = if variants.is_empty() {
        "never".to_string()
    } else {
        variants
            .iter()
            .map(|v| format!("'{}'", escape_single_quotes(v)))
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let array_body = variants
        .iter()
        .map(|v| format!("'{}'", escape_single_quotes(v)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut out = String::new();
    out.push_str(&format!("export type {alias} = {union}\n\n"));
    out.push_str(&format!("export const {const_name}: readonly {alias}[] = [{array_body}] as const\n"));
    out
}

fn escape_single_quotes(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Build the full TS file body for one resource.
pub fn build_resource_types(resource: &ResourceState, enums: &[ParsedEnum]) -> String {
    let stem = type_stem_for_resource(resource);

    let mut out = String::new();

    let referenced = collect_resource_enums(resource, enums);
    if !referenced.is_empty() {
        for (name, _) in &referenced {
            let alias = enum_type_alias(name);
            out.push_str(&format!("import type {{ {alias} }} from './{name}'\n"));
        }
        out.push('\n');
    }

    // --- Db struct (only if Db fields exist) ---
    let db_fields: Vec<_> = resource
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Db))
        .collect();
    if !db_fields.is_empty() {
        out.push_str(&format!("export interface {stem} {{\n"));
        for (name, field) in &db_fields {
            let ty = ts_type(field, enums);
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
            let ty = ts_type(field, enums);
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
            let ty = ts_type_always_optional(field, enums);
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
            let ty = ts_type(field, enums);
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
            let ty = ts_type(field, enums);
            out.push_str(&format!("  {}: {ty}\n", name.as_str()));
        }
        out.push_str("}\n\n");
    }

    emit_filter_interface(resource, enums, &mut out);

    let trimmed = out.trim_end_matches('\n');
    format!("{trimmed}\n")
}

fn emit_filter_interface(resource: &ResourceState, enums: &[ParsedEnum], out: &mut String) {
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
        let ty = filter_ts_type(&field.sql_type, kind, enums);
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
            gen_level: crate::state::GenLevel::default(),
        }
    }

    #[test]
    fn emits_public_interface() {
        let r = synth_resource();
        let enums: Vec<ParsedEnum> = Vec::new();
        let body = build_resource_types(&r, &enums);
        assert!(body.contains("export interface UserPublic {"), "UserPublic missing");
        assert!(body.contains("  id: number"), "id field missing");
        assert!(body.contains("  email: string"), "email field missing");
    }

    #[test]
    fn emits_insertable_interface() {
        let r = synth_resource();
        let enums: Vec<ParsedEnum> = Vec::new();
        let body = build_resource_types(&r, &enums);
        assert!(body.contains("export interface UserInsertable {"), "UserInsertable missing");
        assert!(
            !body.contains("  id: number\nexport interface UserInsertable"),
            "id should not be in insertable"
        );
    }

    #[test]
    fn emits_patch_with_optional_fields() {
        let r = synth_resource();
        let enums: Vec<ParsedEnum> = Vec::new();
        let body = build_resource_types(&r, &enums);
        assert!(body.contains("export interface UserPatch {"), "UserPatch missing");
        assert!(body.contains("  email?: string | null"), "email in patch must be optional");
    }

    #[test]
    fn emits_filter_interface_when_list_verb_present() {
        let r = synth_resource();
        let enums: Vec<ParsedEnum> = Vec::new();
        let body = build_resource_types(&r, &enums);
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

    #[test]
    fn build_enum_module_emits_alias_and_values_const() {
        let body = build_enum_module("user_role", &["admin".to_string(), "member".to_string()]);
        assert!(body.contains("export type UserRole = 'admin' | 'member'"), "alias missing");
        assert!(body.contains("export const USER_ROLE_VALUES: readonly UserRole[] = ['admin', 'member'] as const"), "values const missing");
    }

    #[test]
    fn build_enum_module_handles_single_variant() {
        let body = build_enum_module("flag", &["only".to_string()]);
        assert!(body.contains("export type Flag = 'only'"));
        assert!(body.contains("export const FLAG_VALUES: readonly Flag[] = ['only'] as const"));
    }

    #[test]
    fn build_enum_module_escapes_single_quotes_in_variants() {
        let body = build_enum_module("kind", &["it's".to_string()]);
        assert!(body.contains("'it\\'s'"), "single-quote escape missing");
    }

    #[test]
    fn collect_resource_enums_returns_empty_for_plain_resources() {
        let r = synth_resource();
        let enums: Vec<ParsedEnum> = Vec::new();
        assert!(collect_resource_enums(&r, &enums).is_empty());
    }

    #[test]
    fn build_resource_types_emits_enum_alias_when_field_matches() {
        use std::path::PathBuf;
        let mut r = synth_resource();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();
        r.fields.insert(
            FieldName::new("role"),
            FieldState {
                sql_type: SqlType::new("UserRole"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        let enums = vec![ParsedEnum {
            name: "user_role".to_string(),
            variants: vec!["admin".to_string(), "member".to_string()],
            source_file: PathBuf::from("/tmp/dummy.sql"),
        }];
        let body = build_resource_types(&r, &enums);
        assert!(body.contains("import type { UserRole } from './user_role'"), "enum import missing: {body}");
        assert!(body.contains("  role: UserRole"), "role field with enum alias missing");
    }

    #[test]
    fn collect_resource_enums_finds_match() {
        use std::path::PathBuf;
        let mut r = synth_resource();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public].into_iter().collect();
        r.fields.insert(
            FieldName::new("status"),
            FieldState {
                sql_type: SqlType::new("TaskStatus"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        let enums = vec![ParsedEnum {
            name: "task_status".to_string(),
            variants: vec!["pending".to_string(), "active".to_string()],
            source_file: PathBuf::from("/tmp/dummy.sql"),
        }];
        let collected = collect_resource_enums(&r, &enums);
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].0, "task_status");
    }
}
