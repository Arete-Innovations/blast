use std::collections::BTreeSet;

use crate::{
    codegen::{
        enums::{
            render::enum_type_name,
            scan::ParsedEnum,
        },
        structs::naming::type_stem_for_resource,
    },
    state::{FieldKind, FieldName, FieldState, FieldVariant, ResourceState, SqlType, Verb},
};

/// Look up a `ParsedEnum` whose pascalized name matches the field's declared
/// `sql_type` (case-insensitive). Returns `None` for non-enum fields.
/// True when a field has no UI representation in the create form. Hidden
/// + FromSession both fall in this bucket: form skips signal+input, struct
/// literal still references the field with a placeholder default.
pub fn is_hidden_kind(field: &FieldState) -> bool {
    matches!(field.kind, FieldKind::Hidden | FieldKind::FromSession(_))
}

/// True when a field should render as a `<textarea>` instead of `<input>`.
pub fn is_textarea_kind(field: &FieldState) -> bool {
    matches!(field.kind, FieldKind::Textarea)
}

pub fn find_enum_for_field<'e>(field: &FieldState, enums: &'e [ParsedEnum]) -> Option<&'e ParsedEnum> {
    let want = field.sql_type.as_str().to_ascii_lowercase();
    enums.iter().find(|p| enum_type_name(&p.name).to_ascii_lowercase() == want)
}

pub fn primary_key_field(resource: &ResourceState) -> Option<(&FieldName, &FieldState)> {
    resource.fields.iter().find(|(_, f)| f.primary_key)
}

pub fn pk_rust_type(resource: &ResourceState) -> String {
    match primary_key_field(resource) {
        Some((_pkname, f)) => map_sql_to_rust(&f.sql_type, false),
        None => "i64".to_string(),
    }
}

pub fn fields_for_variant<'a>(resource: &'a ResourceState, variant: FieldVariant) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource.fields.iter().filter(|(_pair_name, f)| f.variants.contains(&variant)).collect()
}

pub fn map_sql_to_rust(sql: &SqlType, nullable: bool) -> String {
    let base = match sql.as_str().to_ascii_lowercase().as_str() {
        "bool" | "boolean" => "bool",
        "int2" | "smallint" | "smallserial" => "i16",
        "int4" | "integer" | "serial" => "i32",
        "int8" | "bigint" | "bigserial" => "i64",
        "float4" | "real" => "f32",
        "float8" | "double" | "double precision" => "f64",
        "numeric" | "decimal" => "rust_decimal::Decimal",
        "text" | "varchar" | "bpchar" | "char" | "citext" => "String",
        "uuid" => "uuid::Uuid",
        "bytea" => "Vec<u8>",
        "timestamp" => "chrono::NaiveDateTime",
        "timestamptz" => "chrono::DateTime<chrono::Utc>",
        "date" => "chrono::NaiveDate",
        "time" => "chrono::NaiveTime",
        "json" | "jsonb" => "serde_json::Value",
        _other => "String",
    };
    match nullable {
        true => format!("Option<{base}>"),
        false => base.to_string(),
    }
}

pub enum InputKind {
    TextLine,
    Number,
    Datetime,
    Date,
    Bool,
    Textarea,
    Enum,
}

pub fn classify_input(field: &FieldState, enums: &[ParsedEnum]) -> InputKind {
    if matches!(field.kind, FieldKind::Textarea) {
        return InputKind::Textarea;
    }
    if find_enum_for_field(field, enums).is_some() {
        return InputKind::Enum;
    }
    let lowered = field.sql_type.as_str().to_ascii_lowercase();
    match lowered.as_str() {
        "bool" | "boolean" => InputKind::Bool,
        "int2" | "smallint" | "smallserial" | "int4" | "integer" | "serial" | "int8" | "bigint" | "bigserial" | "float4" | "real" | "float8" | "double" | "double precision" | "numeric" | "decimal" => {
            InputKind::Number
        }
        "timestamp" | "timestamptz" => InputKind::Datetime,
        "date" => InputKind::Date,
        "json" | "jsonb" => InputKind::Textarea,
        _other => InputKind::TextLine,
    }
}

pub fn looks_like_password(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered.contains("password") || lowered == "pwd"
}

pub fn looks_like_email(name: &str) -> bool {
    name.eq_ignore_ascii_case("email")
}

pub fn looks_like_url(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered == "url" || lowered.ends_with("_url") || lowered.contains("homepage") || lowered.contains("website")
}

fn field_has_rules(field: &FieldState) -> bool {
    !field.validators.is_empty()
}

fn any_textline_with_rules(fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> bool {
    for (_name, field) in fields {
        if is_hidden_kind(field) {
            continue;
        }
        if !field_has_rules(field) {
            continue;
        }
        if matches!(classify_input(field, enums), InputKind::TextLine) {
            return true;
        }
    }
    false
}

pub fn render_create_form(resource: &ResourceState, enums: &[ParsedEnum]) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let component_name = format!("{stem}CreateForm");
    let insertable_type = format!("{stem}Insertable");
    let public_type = format!("{stem}Public");

    let insertable_fields: Vec<(&FieldName, &FieldState)> = fields_for_variant(resource, FieldVariant::Insertable).into_iter().filter(|(_pair_name, f)| !f.primary_key).collect();

    let pk_field_name: String = match primary_key_field(resource) {
        Some((pk_name, _)) => pk_name.as_str().to_string(),
        None => "id".to_string(), // allow: best-effort fallback for resources without a declared PK
    };

    let used_enum_types: BTreeSet<String> = collect_used_enum_types(&insertable_fields, enums);

    let mut out = String::new();
    out.push_str("use std::collections::HashMap;\n");
    out.push_str("use leptos::ev::SubmitEvent;\n");
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::task::spawn_local;\n");
    out.push('\n');
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{insertable_type}, {public_type}}};\n"));
    for ty in &used_enum_types {
        out.push_str(&format!("use crate::structs::generated::enums::{ty};\n"));
    }
    out.push_str("use crate::structs::vendored::validators::Validate;\n");
    out.push_str("use crate::structs::vendored::leptos::{ButtonKind, RouteName};\n");
    let mut components_imports: Vec<&str> = vec!["Button", "ErrorBanner", "FieldError", "LinkButton"];
    if any_textline_with_rules(&insertable_fields, enums) {
        components_imports.push("ValidatedInput");
    }
    components_imports.sort();
    out.push_str(&format!("use crate::views::components::{{{}}};\n", components_imports.join(", ")));
    out.push_str("use crate::views::signals::dispatch_form_error;\n");
    out.push_str("use crate::views::signals::nav::use_blocking_navigate;\n");
    out.push_str("use crate::views::signals::toast;\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::do_{table}_create;\n"));
    out.push('\n');

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component_name}() -> impl IntoView {{\n"));
    out.push_str(&format!("    let cancel_href = RouteName::ResourceList(\"{table}\").path().to_string();\n"));
    out.push_str("    let navigate = StoredValue::new_local(use_blocking_navigate());\n\n");

    for (name, field) in &insertable_fields {
        if is_hidden_kind(field) {
            continue;
        }
        out.push_str(&render_signal_decl(name.as_str(), field, enums));
    }
    out.push_str("    let pending: RwSignal<bool> = RwSignal::new(false);\n");
    out.push_str("    let last_error: RwSignal<Option<MeltDown>> = RwSignal::new(None);\n");
    out.push_str("    let field_errors: RwSignal<HashMap<String, String>> = RwSignal::new(HashMap::new());\n");
    out.push('\n');

    out.push_str("    let on_submit = move |ev: SubmitEvent| {\n");
    out.push_str("        ev.prevent_default();\n");
    out.push_str("        if pending.get_untracked() {\n");
    out.push_str("            return;\n");
    out.push_str("        }\n");
    out.push_str("        field_errors.update(|m| m.clear());\n");
    out.push_str("        last_error.set(None);\n");
    out.push_str(&format!("        let parsed_result: ::std::result::Result<{insertable_type}, MeltDown> = "));
    out.push_str(&render_build_insertable(resource, &insertable_fields, enums));
    out.push_str(";\n");
    out.push_str(&format!("        let parsed: {insertable_type} = match parsed_result {{\n"));
    out.push_str("            Ok(p) => p,\n");
    out.push_str("            Err(err) => {\n");
    out.push_str("                err.log();\n");
    out.push_str("                toast::error(err.user_message());\n");
    out.push_str("                dispatch_form_error(err, field_errors, last_error);\n");
    out.push_str("                return;\n");
    out.push_str("            }\n");
    out.push_str("        };\n");
    out.push_str("        match parsed.check() {\n");
    out.push_str("            Ok(()) => {}\n");
    out.push_str("            Err(err) => {\n");
    out.push_str("                err.log();\n");
    out.push_str("                toast::error(err.user_message());\n");
    out.push_str("                dispatch_form_error(err, field_errors, last_error);\n");
    out.push_str("                return;\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("        pending.set(true);\n");
    out.push_str("        spawn_local(async move {\n");
    out.push_str(&format!("            let outcome = do_{table}_create(parsed).await;\n"));
    out.push_str("            pending.set(false);\n");
    out.push_str("            match outcome {\n");
    out.push_str(&format!(
        "                Ok(record) => {{\n                    toast::success(\"{stem} created.\");\n                    let path = RouteName::ResourceDetail(\"{table}\", record.{pk_field_name}).path().to_string();\n                    navigate.with_value(|nav| nav(&path));\n                }}\n"
    ));
    out.push_str("                Err(err) => {\n");
    out.push_str("                    err.log();\n");
    out.push_str("                    toast::error(err.user_message());\n");
    out.push_str("                    dispatch_form_error(err, field_errors, last_error);\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("        });\n");
    out.push_str("    };\n");
    out.push('\n');

    out.push_str(&format!("    view! {{\n        <form class=\"crud-form {table}-create-form\" on:submit=on_submit>\n"));
    for (name, field) in &insertable_fields {
        if is_hidden_kind(field) {
            continue;
        }
        out.push_str(&render_field_view(name.as_str(), field, enums, &insertable_type));
    }
    out.push_str("            {move || last_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}\n");
    out.push_str("            <div class=\"crud-form__actions\">\n");
    out.push_str("                <LinkButton href=cancel_href.clone() kind=ButtonKind::Ghost>\"Cancel\"</LinkButton>\n");
    out.push_str("                <Button kind=ButtonKind::Primary kind_attr=\"submit\".to_string() disabled=Signal::derive(move || pending.get())>\n");
    out.push_str("                    {move || match pending.get() {\n");
    out.push_str("                        true => \"Saving...\",\n");
    out.push_str("                        false => \"Create\",\n");
    out.push_str("                    }}\n");
    out.push_str("                </Button>\n");
    out.push_str("            </div>\n");
    out.push_str("        </form>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

pub fn render_edit_form(resource: &ResourceState, enums: &[ParsedEnum]) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let component_name = format!("{stem}EditForm");
    let patch_type = format!("{stem}Patch");
    let public_type = format!("{stem}Public");
    let pk_ty = pk_rust_type(resource);

    let patch_fields: Vec<(&FieldName, &FieldState)> = fields_for_variant(resource, FieldVariant::Patch).into_iter().filter(|(_pair_name, f)| !f.primary_key).collect();

    let used_enum_types: BTreeSet<String> = collect_used_enum_types(&patch_fields, enums);

    let mut out = String::new();
    out.push_str("use std::collections::HashMap;\n");
    out.push_str("use leptos::ev::SubmitEvent;\n");
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::task::spawn_local;\n");
    out.push('\n');
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{patch_type}, {public_type}}};\n"));
    for ty in &used_enum_types {
        out.push_str(&format!("use crate::structs::generated::enums::{ty};\n"));
    }
    out.push_str("use crate::structs::vendored::validators::Validate;\n");
    out.push_str("use crate::structs::vendored::leptos::{ButtonKind, RouteName};\n");
    let mut components_imports: Vec<&str> = vec!["Button", "ErrorBanner", "FieldError", "LinkButton"];
    if any_textline_with_rules(&patch_fields, enums) {
        components_imports.push("ValidatedInput");
    }
    components_imports.sort();
    out.push_str(&format!("use crate::views::components::{{{}}};\n", components_imports.join(", ")));
    out.push_str("use crate::views::signals::dispatch_form_error;\n");
    out.push_str("use crate::views::signals::nav::use_blocking_navigate;\n");
    out.push_str("use crate::views::signals::toast;\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::do_{table}_update;\n"));
    out.push('\n');

    let pk_field_name: String = match primary_key_field(resource) {
        Some((pk_name, _f)) => pk_name.as_str().to_string(),
        None => "id".to_string(), // unreachable: edit_form is only emitted when primary_key_field(...).is_some()
    };

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component_name}(initial: {public_type}) -> impl IntoView {{\n"));
    out.push_str(&format!("    let row_id: {pk_ty} = initial.{pk_field_name}.clone();\n"));
    out.push_str(&format!(
        "    let cancel_href = RouteName::ResourceDetail(\"{table}\", row_id.clone()).path().to_string();\n"
    ));
    out.push_str("    let navigate = StoredValue::new_local(use_blocking_navigate());\n\n");

    for (name, field) in &patch_fields {
        out.push_str(&render_signal_decl_initialized(name.as_str(), field, enums));
    }
    out.push_str("    let pending: RwSignal<bool> = RwSignal::new(false);\n");
    out.push_str("    let last_error: RwSignal<Option<MeltDown>> = RwSignal::new(None);\n");
    out.push_str("    let field_errors: RwSignal<HashMap<String, String>> = RwSignal::new(HashMap::new());\n");
    out.push('\n');

    out.push_str("    let on_submit = move |ev: SubmitEvent| {\n");
    out.push_str("        ev.prevent_default();\n");
    out.push_str("        if pending.get_untracked() {\n");
    out.push_str("            return;\n");
    out.push_str("        }\n");
    out.push_str("        field_errors.update(|m| m.clear());\n");
    out.push_str("        last_error.set(None);\n");
    out.push_str(&format!("        let patch_result: ::std::result::Result<{patch_type}, MeltDown> = "));
    out.push_str(&render_build_patch(resource, &patch_fields, enums));
    out.push_str(";\n");
    out.push_str(&format!("        let patch: {patch_type} = match patch_result {{\n"));
    out.push_str("            Ok(p) => p,\n");
    out.push_str("            Err(err) => {\n");
    out.push_str("                err.log();\n");
    out.push_str("                toast::error(err.user_message());\n");
    out.push_str("                dispatch_form_error(err, field_errors, last_error);\n");
    out.push_str("                return;\n");
    out.push_str("            }\n");
    out.push_str("        };\n");
    out.push_str("        match patch.check() {\n");
    out.push_str("            Ok(()) => {}\n");
    out.push_str("            Err(err) => {\n");
    out.push_str("                err.log();\n");
    out.push_str("                toast::error(err.user_message());\n");
    out.push_str("                dispatch_form_error(err, field_errors, last_error);\n");
    out.push_str("                return;\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("        pending.set(true);\n");
    out.push_str("        let captured_id = row_id.clone();\n");
    out.push_str("        spawn_local(async move {\n");
    out.push_str(&format!("            let outcome = do_{table}_update(captured_id, patch).await;\n"));
    out.push_str("            pending.set(false);\n");
    out.push_str("            match outcome {\n");
    out.push_str(&format!(
        "                Ok(record) => {{\n                    toast::success(\"{stem} saved.\");\n                    let path = RouteName::ResourceDetail(\"{table}\", record.{pk_field_name}).path().to_string();\n                    navigate.with_value(|nav| nav(&path));\n                }}\n"
    ));
    out.push_str("                Err(err) => {\n");
    out.push_str("                    err.log();\n");
    out.push_str("                    toast::error(err.user_message());\n");
    out.push_str("                    dispatch_form_error(err, field_errors, last_error);\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("        });\n");
    out.push_str("    };\n");
    out.push('\n');

    out.push_str(&format!("    view! {{\n        <form class=\"crud-form {table}-edit-form\" on:submit=on_submit>\n"));
    for (name, field) in &patch_fields {
        out.push_str(&render_field_view(name.as_str(), field, enums, &patch_type));
    }
    out.push_str("            {move || last_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}\n");
    out.push_str("            <div class=\"crud-form__actions\">\n");
    out.push_str("                <LinkButton href=cancel_href.clone() kind=ButtonKind::Ghost>\"Cancel\"</LinkButton>\n");
    out.push_str("                <Button kind=ButtonKind::Primary kind_attr=\"submit\".to_string() disabled=Signal::derive(move || pending.get())>\n");
    out.push_str("                    {move || match pending.get() {\n");
    out.push_str("                        true => \"Saving...\",\n");
    out.push_str("                        false => \"Save\",\n");
    out.push_str("                    }}\n");
    out.push_str("                </Button>\n");
    out.push_str("            </div>\n");
    out.push_str("        </form>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn render_signal_decl_initialized(name: &str, field: &FieldState, enums: &[ParsedEnum]) -> String {
    let kind = classify_input(field, enums);
    let lowered = field.sql_type.as_str().to_ascii_lowercase();
    let nul_init = |expr_on_v: &str| match field.nullable {
        true => format!("    let {name}: RwSignal<String> = RwSignal::new(match &initial.{name} {{ Some(v) => {expr_on_v}, None => String::new() }});\n"),
        false => format!("    let {name}: RwSignal<String> = RwSignal::new({{ let v = &initial.{name}; {expr_on_v} }});\n"),
    };
    match kind {
        InputKind::Bool => format!("    let {name} = RwSignal::new(initial.{name});\n"),
        InputKind::Number => nul_init("v.to_string()"),
        InputKind::Datetime => match lowered.as_str() {
            "timestamptz" => nul_init("v.to_rfc3339()"),
            _other => nul_init("v.format(\"%Y-%m-%dT%H:%M:%S\").to_string()"),
        },
        InputKind::Date => nul_init("v.format(\"%Y-%m-%d\").to_string()"),
        InputKind::Enum => format!("    let {name}: RwSignal<String> = RwSignal::new(initial.{name}.as_str().to_string());\n"),
        _stringy => match lowered.as_str() {
            "uuid" => format!("    let {name}: RwSignal<String> = RwSignal::new(initial.{name}.to_string());\n"),
            "json" | "jsonb" => format!(
                "    let {name}: RwSignal<String> = RwSignal::new(match ::serde_json::to_string(&initial.{name}) {{ Ok(s) => s, Err(parse_err) => {{ crate::cata_log!(Debug, format!(\"json initial serialize failed: {{}}\", parse_err)); String::new() }} }});\n"
            ),
            _stringy_text => match field.nullable {
                true => format!("    let {name}: RwSignal<String> = RwSignal::new(match &initial.{name} {{ Some(s) => s.clone(), None => String::new() }});\n"),
                false => format!("    let {name}: RwSignal<String> = RwSignal::new(initial.{name}.clone());\n"),
            },
        },
    }
}

fn render_signal_decl(name: &str, field: &FieldState, enums: &[ParsedEnum]) -> String {
    let kind = classify_input(field, enums);
    match kind {
        InputKind::Bool => format!("    let {name} = RwSignal::new(false);\n"),
        _other => format!("    let {name}: RwSignal<String> = RwSignal::new(String::new());\n"),
    }
}

fn render_build_insertable(resource: &ResourceState, fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let stem = type_stem_for_resource(resource);
    let insertable_type = format!("{stem}Insertable");
    let mut out = String::new();
    out.push_str(&format!("(|| -> ::std::result::Result<{insertable_type}, MeltDown> {{\n"));
    for (name, field) in fields {
        out.push_str(&render_field_parse_let(name.as_str(), field, enums));
    }
    out.push_str(&format!("            Ok({insertable_type} {{\n"));
    for (name, field) in fields {
        out.push_str(&render_field_struct_assign(name.as_str(), field, false));
    }
    out.push_str("            })\n");
    out.push_str("        })()");
    out
}

fn render_build_patch(resource: &ResourceState, fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let stem = type_stem_for_resource(resource);
    let patch_type = format!("{stem}Patch");
    let mut out = String::new();
    out.push_str(&format!("(|| -> ::std::result::Result<{patch_type}, MeltDown> {{\n"));
    for (name, field) in fields {
        out.push_str(&render_field_parse_let(name.as_str(), field, enums));
    }
    out.push_str(&format!("            Ok({patch_type} {{\n"));
    for (name, field) in fields {
        out.push_str(&render_field_struct_assign(name.as_str(), field, true));
    }
    out.push_str("            })\n");
    out.push_str("        })()");
    out
}

fn render_field_parse_let(name: &str, field: &FieldState, enums: &[ParsedEnum]) -> String {
    if is_hidden_kind(field) {
        // No UI for this field. Emit a placeholder so the Insertable struct
        // literal still type-checks. Server (HTTP create handler) overwrites
        // the value before validation when kind = FromSession.
        let kind = classify_input(field, enums);
        return match kind {
            InputKind::Bool => format!("            let {name}_val: bool = false;\n"),
            InputKind::Number => {
                let target = number_target(field);
                format!("            let {name}_val: {target} = 0;\n")
            }
            InputKind::Datetime => {
                let lowered = field.sql_type.as_str().to_ascii_lowercase();
                match lowered.as_str() {
                    "timestamptz" => format!(
                        "            let {name}_val: chrono::DateTime<chrono::Utc> = chrono::Utc::now();\n"
                    ),
                    _other => format!(
                        "            let {name}_val: chrono::NaiveDateTime = chrono::Utc::now().naive_utc();\n"
                    ),
                }
            }
            InputKind::Date => format!(
                "            let {name}_val: chrono::NaiveDate = chrono::Utc::now().date_naive();\n"
            ),
            InputKind::Enum => {
                let parsed = match find_enum_for_field(field, enums) {
                    Some(p) => p,
                    None => return format!("            let {name}_val: String = String::new();\n", name = name),
                };
                let ty = enum_type_name(&parsed.name);
                format!("            let {name}_val: {ty} = ::std::default::Default::default();\n")
            }
            _stringy => {
                let lowered = field.sql_type.as_str().to_ascii_lowercase();
                match lowered.as_str() {
                    "uuid" => format!("            let {name}_val: uuid::Uuid = uuid::Uuid::nil();\n"),
                    "json" | "jsonb" => format!(
                        "            let {name}_val: serde_json::Value = serde_json::Value::Null;\n"
                    ),
                    _stringy_text => format!("            let {name}_val: String = String::new();\n"),
                }
            }
        };
    }
    let kind = classify_input(field, enums);
    let raw_var = format!("{name}_raw");
    match kind {
        InputKind::Bool => {
            format!("            let {name}_val: bool = {name}.get_untracked();\n")
        }
        InputKind::Number => {
            let target = number_target(field);
            format!(
                "            let {raw_var}: String = {name}.get_untracked();\n            let {name}_val: {target} = match {raw_var}.parse::<{target}>() {{\n                Ok(v) => v,\n                Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a number: {{}}\", parse_err))),\n            }};\n",
                target = target,
                raw_var = raw_var,
                name = name,
            )
        }
        InputKind::Datetime => {
            let lowered = field.sql_type.as_str().to_ascii_lowercase();
            match lowered.as_str() {
                "timestamptz" => format!(
                    "            let {raw_var}: String = {name}.get_untracked();\n            let {name}_val: chrono::DateTime<chrono::Utc> = match chrono::DateTime::parse_from_rfc3339(&{raw_var}) {{\n                Ok(v) => v.with_timezone(&chrono::Utc),\n                Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid RFC3339 datetime: {{}}\", parse_err))),\n            }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
                _other => format!(
                    "            let {raw_var}: String = {name}.get_untracked();\n            let {name}_val: chrono::NaiveDateTime = match chrono::NaiveDateTime::parse_from_str(&{raw_var}, \"%Y-%m-%dT%H:%M:%S\") {{\n                Ok(v) => v,\n                Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid datetime: {{}}\", parse_err))),\n            }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
            }
        }
        InputKind::Date => format!(
            "            let {raw_var}: String = {name}.get_untracked();\n            let {name}_val: chrono::NaiveDate = match chrono::NaiveDate::parse_from_str(&{raw_var}, \"%Y-%m-%d\") {{\n                Ok(v) => v,\n                Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid date: {{}}\", parse_err))),\n            }};\n",
            raw_var = raw_var,
            name = name,
        ),
        InputKind::Enum => {
            let parsed = match find_enum_for_field(field, enums) {
                Some(p) => p,
                None => return format!("            let {name}_val: String = {name}.get_untracked();\n", name = name),
            };
            let ty = enum_type_name(&parsed.name);
            format!(
                "            let {raw_var}: String = {name}.get_untracked();\n            let {name}_val: {ty} = match {ty}::parse(&{raw_var}) {{\n                Ok(v) => v,\n                Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"invalid {ty}: {{}}\", parse_err))),\n            }};\n",
                raw_var = raw_var,
                name = name,
                ty = ty,
            )
        }
        _stringy => {
            let lowered = field.sql_type.as_str().to_ascii_lowercase();
            match lowered.as_str() {
                "uuid" => format!(
                    "            let {raw_var}: String = {name}.get_untracked();\n            let {name}_val: uuid::Uuid = match uuid::Uuid::parse_str(&{raw_var}) {{\n                Ok(v) => v,\n                Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid UUID: {{}}\", parse_err))),\n            }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
                "json" | "jsonb" => format!(
                    "            let {raw_var}: String = {name}.get_untracked();\n            let {name}_val: serde_json::Value = match serde_json::from_str::<serde_json::Value>(&{raw_var}) {{\n                Ok(v) => v,\n                Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be valid JSON: {{}}\", parse_err))),\n            }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
                _stringy_text => format!("            let {name}_val: String = {name}.get_untracked();\n", name = name),
            }
        }
    }
}

fn render_field_struct_assign(name: &str, field: &FieldState, is_patch: bool) -> String {
    if is_patch {
        return format!("                {name}: Some({name}_val),\n");
    }
    if field.nullable {
        return format!("                {name}: Some({name}_val),\n");
    }
    format!("                {name}: {name}_val,\n")
}

fn number_target(field: &FieldState) -> &'static str {
    let lowered = field.sql_type.as_str().to_ascii_lowercase();
    match lowered.as_str() {
        "int2" | "smallint" | "smallserial" => "i16",
        "int4" | "integer" | "serial" => "i32",
        "int8" | "bigint" | "bigserial" => "i64",
        "float4" | "real" => "f32",
        "float8" | "double" | "double precision" => "f64",
        _other => "i64",
    }
}

fn render_field_view(name: &str, field: &FieldState, enums: &[ParsedEnum], dto_type: &str) -> String {
    let kind = classify_input(field, enums);
    let label_text = pretty_label(name);
    let mut out = String::new();
    out.push_str("            <label class=\"crud-form__field\">\n");
    out.push_str(&format!("                <span class=\"crud-form__label\">\"{label_text}\"</span>\n"));

    match kind {
        InputKind::Bool => {
            out.push_str(&format!(
                "                <input\n                    type=\"checkbox\"\n                    prop:checked=move || {name}.get()\n                    on:change=move |ev| {name}.set(event_target_checked(&ev))\n                />\n",
            ));
        }
        InputKind::Textarea => {
            out.push_str(&format!(
                "                <textarea\n                    prop:value=move || {name}.get()\n                    on:input=move |ev| {name}.set(event_target_value(&ev))\n                />\n",
            ));
        }
        InputKind::Datetime => {
            out.push_str(&format!(
                "                <input\n                    type=\"datetime-local\"\n                    prop:value=move || {name}.get()\n                    on:input=move |ev| {name}.set(event_target_value(&ev))\n                />\n",
            ));
        }
        InputKind::Date => {
            out.push_str(&format!(
                "                <input\n                    type=\"date\"\n                    prop:value=move || {name}.get()\n                    on:input=move |ev| {name}.set(event_target_value(&ev))\n                />\n",
            ));
        }
        InputKind::Number => {
            out.push_str(&format!(
                "                <input\n                    type=\"text\"\n                    inputmode=\"numeric\"\n                    prop:value=move || {name}.get()\n                    on:input=move |ev| {name}.set(event_target_value(&ev))\n                />\n",
            ));
        }
        InputKind::Enum => match find_enum_for_field(field, enums) {
            Some(parsed) => {
                out.push_str(&format!(
                    "                <select\n                    prop:value=move || {name}.get()\n                    on:change=move |ev| {name}.set(event_target_value(&ev))\n                >\n",
                ));
                for variant in &parsed.variants {
                    let escaped = variant.replace('\\', "\\\\").replace('"', "\\\"");
                    out.push_str(&format!("                    <option value=\"{escaped}\">\"{escaped}\"</option>\n"));
                }
                out.push_str("                </select>\n");
            }
            None => {
                out.push_str(&format!(
                    "                <input\n                    type=\"text\"\n                    prop:value=move || {name}.get()\n                    on:input=move |ev| {name}.set(event_target_value(&ev))\n                />\n",
                ));
            }
        },
        InputKind::TextLine => {
            let html_type = if looks_like_password(name) {
                "password"
            } else if looks_like_email(name) {
                "email"
            } else if looks_like_url(name) {
                "url"
            } else {
                "text"
            };
            if field_has_rules(field) {
                out.push_str(&format!(
                    "                <ValidatedInput\n                    field=\"{name}\".to_string()\n                    rules={{<{dto_type} as Validate>::rules_for(\"{name}\")}}\n                    value={name}\n                    input_type=\"{html_type}\".to_string()\n                />\n",
                ));
            } else {
                out.push_str(&format!(
                    "                <input\n                    type=\"{html_type}\"\n                    prop:value=move || {name}.get()\n                    on:input=move |ev| {name}.set(event_target_value(&ev))\n                />\n",
                ));
            }
        }
    }

    out.push_str("            </label>\n");
    out.push_str(&format!(
        "            <FieldError message=Signal::derive(move || field_errors.with(|m| m.get(\"{name}\").cloned()))/>\n"
    ));
    out
}

fn collect_used_enum_types(fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for (_, field) in fields {
        match find_enum_for_field(field, enums) {
            Some(parsed) => {
                set.insert(enum_type_name(&parsed.name));
            }
            None => {}
        }
    }
    set
}

fn pretty_label(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut first = true;
    let mut prev_underscore = false;
    for ch in name.chars() {
        if ch == '_' {
            out.push(' ');
            prev_underscore = true;
            continue;
        }
        if first {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            first = false;
            prev_underscore = false;
            continue;
        }
        if prev_underscore {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            prev_underscore = false;
            continue;
        }
        out.push(ch);
    }
    out
}

pub fn render_resource_form_barrel(resource: &ResourceState) -> String {
    let stem = type_stem_for_resource(resource);
    let has_create = resource.verbs.contains_key(&Verb::Create);
    let has_edit = resource.verbs.contains_key(&Verb::Update) && primary_key_field(resource).is_some();

    let mut out = String::new();
    if has_create {
        out.push_str("pub mod create_form;\n");
    }
    if has_edit {
        out.push_str("pub mod edit_form;\n");
    }
    if has_create || has_edit {
        out.push('\n');
    }
    if has_create {
        out.push_str(&format!("pub use create_form::{stem}CreateForm;\n"));
    }
    if has_edit {
        out.push_str(&format!("pub use edit_form::{stem}EditForm;\n"));
    }
    out
}

pub fn render_data_stub(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let insertable_type = format!("{stem}Insertable");
    let patch_type = format!("{stem}Patch");
    let public_type = format!("{stem}Public");
    let pk_ty = pk_rust_type(resource);

    let mut out = String::new();
    out.push_str("use crate::meltdown::{MeltDown, MeltType};\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{insertable_type}, {patch_type}, {public_type}}};\n"));
    out.push('\n');

    if resource.verbs.contains_key(&Verb::Create) {
        out.push_str(&format!("pub async fn do_{table}_create(input: {insertable_type}) -> ::std::result::Result<{public_type}, MeltDown> {{\n"));
        out.push_str("    drop(input);\n");
        out.push_str(&format!(
            "    Err(MeltDown::new(MeltType::Unexpected(\"not_implemented\".to_string()), \"do_{table}_create not yet implemented\"))\n"
        ));
        out.push_str("}\n");
        out.push('\n');
    }

    if resource.verbs.contains_key(&Verb::Update) && primary_key_field(resource).is_some() {
        out.push_str(&format!(
            "pub async fn do_{table}_update(id: {pk_ty}, patch: {patch_type}) -> ::std::result::Result<{public_type}, MeltDown> {{\n"
        ));
        out.push_str("    drop(id);\n");
        out.push_str("    drop(patch);\n");
        out.push_str(&format!(
            "    Err(MeltDown::new(MeltType::Unexpected(\"not_implemented\".to_string()), \"do_{table}_update not yet implemented\"))\n"
        ));
        out.push_str("}\n");
        out.push('\n');
    }

    out
}

pub fn render_data_barrel(tables: &[&str]) -> String {
    let mut sorted: Vec<&&str> = tables.iter().collect();
    sorted.sort();
    let mut out = String::new();
    for t in &sorted {
        out.push_str(&format!("pub mod {t};\n"));
    }
    out
}

pub fn render_top_forms_barrel(tables: &[&str]) -> String {
    let mut sorted: Vec<&&str> = tables.iter().collect();
    sorted.sort();
    let mut out = String::new();
    for t in &sorted {
        out.push_str(&format!("pub mod {t};\n"));
    }
    out
}

pub fn render_components_generated_mod() -> String {
    "pub mod forms;\n".to_string()
}
