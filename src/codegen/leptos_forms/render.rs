use std::collections::BTreeSet;

use crate::{
    codegen::{
        enums::{
            render::enum_type_name,
            scan::ParsedEnum,
        },
        structs::naming::type_stem_for_resource,
    },
    state::{FieldName, FieldState, FieldVariant, ResourceState, SqlType, Verb},
};

/// Look up a `ParsedEnum` whose pascalized name matches the field's declared
/// `sql_type` (case-insensitive). Returns `None` for non-enum fields.
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

pub fn render_create_form(resource: &ResourceState, enums: &[ParsedEnum]) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let component_name = format!("{stem}CreateForm");
    let insertable_type = format!("{stem}Insertable");
    let public_type = format!("{stem}Public");

    let insertable_fields: Vec<(&FieldName, &FieldState)> = fields_for_variant(resource, FieldVariant::Insertable).into_iter().filter(|(_pair_name, f)| !f.primary_key).collect();

    let used_enum_types: BTreeSet<String> = collect_used_enum_types(&insertable_fields, enums);
    let has_enum = !used_enum_types.is_empty();

    let mut out = String::new();
    out.push_str("use leptos::ev::SubmitEvent;\n");
    out.push_str("use leptos::prelude::*;\n");
    out.push_str(&render_thaw_imports(has_enum));
    out.push('\n');
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{insertable_type}, {public_type}}};\n"));
    for ty in &used_enum_types {
        out.push_str(&format!("use crate::structs::generated::enums::{ty};\n"));
    }
    out.push_str(&format!("use crate::structs::generated::validators::{table}::validate_{table}_insertable;\n"));
    out.push_str("use crate::transport::leptos::components::ErrorBanner;\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::do_{table}_create;\n"));
    out.push('\n');

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component_name}() -> impl IntoView {{\n"));

    for (name, field) in &insertable_fields {
        out.push_str(&render_signal_decl(name.as_str(), field, enums));
    }
    out.push('\n');

    out.push_str(&format!(
        "    let create_action: Action<(), ::std::result::Result<{public_type}, MeltDown>> = Action::new(move |_input: &()| {{\n"
    ));
    out.push_str("        async move {\n");
    out.push_str(&format!("            let parsed: {insertable_type} = "));
    out.push_str(&render_build_insertable(resource, &insertable_fields, enums));
    out.push_str(";\n");
    out.push_str(&format!("            validate_{table}_insertable(&parsed)?;\n"));
    out.push_str(&format!("            do_{table}_create(parsed).await\n"));
    out.push_str("        }\n");
    out.push_str("    });\n");
    out.push('\n');

    out.push_str("    let pending = create_action.pending();\n");
    out.push_str("    let value = create_action.value();\n");
    out.push('\n');

    out.push_str("    let on_submit = move |ev: SubmitEvent| {\n");
    out.push_str("        ev.prevent_default();\n");
    out.push_str("        create_action.dispatch(());\n");
    out.push_str("    };\n");
    out.push('\n');

    out.push_str(&format!("    view! {{\n        <form class=\"{table}-create-form\" on:submit=on_submit>\n"));
    for (name, field) in &insertable_fields {
        out.push_str(&render_field_view(name.as_str(), field, enums));
    }
    out.push_str("            {move || match value.get() {\n");
    out.push_str("                Some(Err(error)) => view! { <ErrorBanner error=error/> }.into_any(),\n");
    out.push_str("                Some(Ok(_ok)) => view! { <span/> }.into_any(),\n");
    out.push_str("                None => view! { <span/> }.into_any(),\n");
    out.push_str("            }}\n");
    out.push_str("            <button type=\"submit\" prop:disabled=move || pending.get()>\n");
    out.push_str("                {move || match pending.get() {\n");
    out.push_str("                    true => \"Saving...\",\n");
    out.push_str("                    false => \"Create\",\n");
    out.push_str("                }}\n");
    out.push_str("            </button>\n");
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
    let has_enum = !used_enum_types.is_empty();

    let mut out = String::new();
    out.push_str("use leptos::ev::SubmitEvent;\n");
    out.push_str("use leptos::prelude::*;\n");
    out.push_str(&render_thaw_imports(has_enum));
    out.push('\n');
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{patch_type}, {public_type}}};\n"));
    for ty in &used_enum_types {
        out.push_str(&format!("use crate::structs::generated::enums::{ty};\n"));
    }
    out.push_str(&format!("use crate::structs::generated::validators::{table}::validate_{table}_patch;\n"));
    out.push_str("use crate::transport::leptos::components::ErrorBanner;\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::do_{table}_update;\n"));
    out.push('\n');

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component_name}(initial: {public_type}) -> impl IntoView {{\n"));

    out.push_str(&format!("    let row_id: {pk_ty} = initial.id.clone();\n"));
    for (name, field) in &patch_fields {
        out.push_str(&render_signal_decl(name.as_str(), field, enums));
    }
    out.push('\n');

    out.push_str(&format!("    let update_action: Action<(), ::std::result::Result<{public_type}, MeltDown>> = Action::new(move |_input: &()| {{\n"));
    out.push_str("        let captured_id = row_id.clone();\n");
    out.push_str("        async move {\n");
    out.push_str(&format!("            let patch: {patch_type} = "));
    out.push_str(&render_build_patch(resource, &patch_fields, enums));
    out.push_str(";\n");
    out.push_str(&format!("            validate_{table}_patch(&patch)?;\n"));
    out.push_str(&format!("            do_{table}_update(captured_id, patch).await\n"));
    out.push_str("        }\n");
    out.push_str("    });\n");
    out.push('\n');

    out.push_str("    let pending = update_action.pending();\n");
    out.push_str("    let value = update_action.value();\n");
    out.push('\n');

    out.push_str("    let on_submit = move |ev: SubmitEvent| {\n");
    out.push_str("        ev.prevent_default();\n");
    out.push_str("        update_action.dispatch(());\n");
    out.push_str("    };\n");
    out.push('\n');

    out.push_str(&format!("    view! {{\n        <form class=\"{table}-edit-form\" on:submit=on_submit>\n"));
    for (name, field) in &patch_fields {
        out.push_str(&render_field_view(name.as_str(), field, enums));
    }
    out.push_str("            {move || match value.get() {\n");
    out.push_str("                Some(Err(error)) => view! { <ErrorBanner error=error/> }.into_any(),\n");
    out.push_str("                Some(Ok(_ok)) => view! { <span/> }.into_any(),\n");
    out.push_str("                None => view! { <span/> }.into_any(),\n");
    out.push_str("            }}\n");
    out.push_str("            <button type=\"submit\" prop:disabled=move || pending.get()>\n");
    out.push_str("                {move || match pending.get() {\n");
    out.push_str("                    true => \"Saving...\",\n");
    out.push_str("                    false => \"Save\",\n");
    out.push_str("                }}\n");
    out.push_str("            </button>\n");
    out.push_str("        </form>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
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
    out.push_str("{\n");
    for (name, field) in fields {
        out.push_str(&render_field_parse_let(name.as_str(), field, enums));
    }
    out.push_str(&format!("                {insertable_type} {{\n"));
    for (name, field) in fields {
        out.push_str(&render_field_struct_assign(name.as_str(), field, false));
    }
    out.push_str("                }\n");
    out.push_str("            }");
    out
}

fn render_build_patch(resource: &ResourceState, fields: &[(&FieldName, &FieldState)], enums: &[ParsedEnum]) -> String {
    let stem = type_stem_for_resource(resource);
    let patch_type = format!("{stem}Patch");
    let mut out = String::new();
    out.push_str("{\n");
    for (name, field) in fields {
        out.push_str(&render_field_parse_let(name.as_str(), field, enums));
    }
    out.push_str(&format!("                {patch_type} {{\n"));
    for (name, field) in fields {
        out.push_str(&render_field_struct_assign(name.as_str(), field, true));
    }
    out.push_str("                }\n");
    out.push_str("            }");
    out
}

fn render_field_parse_let(name: &str, field: &FieldState, enums: &[ParsedEnum]) -> String {
    let kind = classify_input(field, enums);
    let raw_var = format!("{name}_raw");
    match kind {
        InputKind::Bool => {
            format!("                let {name}_val: bool = {name}.get_untracked();\n")
        }
        InputKind::Number => {
            let target = number_target(field);
            format!(
                "                let {raw_var}: String = {name}.get_untracked();\n                let {name}_val: {target} = match {raw_var}.parse::<{target}>() {{\n                    Ok(v) => v,\n                    Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a number: {{}}\", parse_err))),\n                }};\n",
                target = target,
                raw_var = raw_var,
                name = name,
            )
        }
        InputKind::Datetime => {
            let lowered = field.sql_type.as_str().to_ascii_lowercase();
            match lowered.as_str() {
                "timestamptz" => format!(
                    "                let {raw_var}: String = {name}.get_untracked();\n                let {name}_val: chrono::DateTime<chrono::Utc> = match chrono::DateTime::parse_from_rfc3339(&{raw_var}) {{\n                    Ok(v) => v.with_timezone(&chrono::Utc),\n                    Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid RFC3339 datetime: {{}}\", parse_err))),\n                }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
                _other => format!(
                    "                let {raw_var}: String = {name}.get_untracked();\n                let {name}_val: chrono::NaiveDateTime = match chrono::NaiveDateTime::parse_from_str(&{raw_var}, \"%Y-%m-%dT%H:%M:%S\") {{\n                    Ok(v) => v,\n                    Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid datetime: {{}}\", parse_err))),\n                }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
            }
        }
        InputKind::Date => format!(
            "                let {raw_var}: String = {name}.get_untracked();\n                let {name}_val: chrono::NaiveDate = match chrono::NaiveDate::parse_from_str(&{raw_var}, \"%Y-%m-%d\") {{\n                    Ok(v) => v,\n                    Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid date: {{}}\", parse_err))),\n                }};\n",
            raw_var = raw_var,
            name = name,
        ),
        InputKind::Enum => {
            let parsed = match find_enum_for_field(field, enums) {
                Some(p) => p,
                None => return format!("                let {name}_val: String = {name}.get_untracked();\n", name = name),
            };
            let ty = enum_type_name(&parsed.name);
            format!(
                "                let {raw_var}: String = {name}.get_untracked();\n                let {name}_val: {ty} = match {ty}::parse(&{raw_var}) {{\n                    Ok(v) => v,\n                    Err(_parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", \"invalid {ty}\")),\n                }};\n",
                raw_var = raw_var,
                name = name,
                ty = ty,
            )
        }
        _stringy => {
            let lowered = field.sql_type.as_str().to_ascii_lowercase();
            match lowered.as_str() {
                "uuid" => format!(
                    "                let {raw_var}: String = {name}.get_untracked();\n                let {name}_val: uuid::Uuid = match uuid::Uuid::parse_str(&{raw_var}) {{\n                    Ok(v) => v,\n                    Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be a valid UUID: {{}}\", parse_err))),\n                }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
                "json" | "jsonb" => format!(
                    "                let {raw_var}: String = {name}.get_untracked();\n                let {name}_val: serde_json::Value = match serde_json::from_str::<serde_json::Value>(&{raw_var}) {{\n                    Ok(v) => v,\n                    Err(parse_err) => return Err(MeltDown::validation_failed_field(\"{name}\", format!(\"must be valid JSON: {{}}\", parse_err))),\n                }};\n",
                    raw_var = raw_var,
                    name = name,
                ),
                _stringy_text => format!("                let {name}_val: String = {name}.get_untracked();\n", name = name),
            }
        }
    }
}

fn render_field_struct_assign(name: &str, field: &FieldState, is_patch: bool) -> String {
    if is_patch {
        return format!("                    {name}: Some({name}_val),\n");
    }
    if field.nullable {
        return format!("                    {name}: Some({name}_val),\n");
    }
    format!("                    {name}: {name}_val,\n")
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

fn render_field_view(name: &str, field: &FieldState, enums: &[ParsedEnum]) -> String {
    let kind = classify_input(field, enums);
    let label_text = pretty_label(name);
    let mut out = String::new();
    out.push_str("            <label>\n");
    out.push_str(&format!("                <span>\"{label_text}\"</span>\n"));

    match kind {
        InputKind::Bool => out.push_str(&format!("                <Checkbox checked={name}/>\n")),
        InputKind::Textarea => out.push_str(&format!("                <Textarea value={name}/>\n")),
        InputKind::Datetime => out.push_str(&format!("                <Input value={name} input_type=InputType::DatetimeLocal/>\n")),
        InputKind::Date => out.push_str(&format!("                <Input value={name} input_type=InputType::Date/>\n")),
        InputKind::Number => out.push_str(&format!("                <Input value={name} input_type=InputType::Text/>\n")),
        InputKind::Enum => match find_enum_for_field(field, enums) {
            Some(parsed) => {
                out.push_str(&format!("                <Combobox value={name}>\n"));
                for variant in &parsed.variants {
                    let escaped = variant.replace('\\', "\\\\").replace('"', "\\\"");
                    out.push_str(&format!("                    <ComboboxOption value=\"{escaped}\".to_string() text=\"{escaped}\".to_string()/>\n"));
                }
                out.push_str("                </Combobox>\n");
            }
            None => out.push_str(&format!("                <Input value={name} input_type=InputType::Text/>\n")),
        },
        InputKind::TextLine => {
            if looks_like_password(name) {
                out.push_str(&format!("                <Input value={name} input_type=InputType::Password/>\n"));
            } else if looks_like_email(name) {
                out.push_str(&format!("                <Input value={name} input_type=InputType::Email/>\n"));
            } else if looks_like_url(name) {
                out.push_str(&format!("                <Input value={name} input_type=InputType::Url/>\n"));
            } else {
                out.push_str(&format!("                <Input value={name} input_type=InputType::Text/>\n"));
            }
        }
    }

    out.push_str("            </label>\n");
    out
}

fn render_thaw_imports(has_enum: bool) -> String {
    match has_enum {
        true => "use thaw::{Checkbox, Combobox, ComboboxOption, Input, InputType, Textarea};\n".to_string(),
        false => "use thaw::{Checkbox, Input, InputType, Textarea};\n".to_string(),
    }
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
    let mut out = String::new();
    if resource.verbs.contains_key(&Verb::Create) {
        out.push_str("pub mod create_form;\n");
    }
    if resource.verbs.contains_key(&Verb::Update) && primary_key_field(resource).is_some() {
        out.push_str("pub mod edit_form;\n");
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
