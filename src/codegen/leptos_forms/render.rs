use crate::{
    codegen::structs::naming::type_stem_for_resource,
    state::{FieldName, FieldState, FieldVariant, ResourceState, SqlType, Verb},
};

pub fn primary_key_field(resource: &ResourceState) -> Option<(&FieldName, &FieldState)> {
    resource.fields.iter().find(|(_, f)| f.primary_key)
}

pub fn pk_rust_type(resource: &ResourceState) -> String {
    match primary_key_field(resource) {
        Some((_, f)) => map_sql_to_rust(&f.sql_type, false),
        _none => "i64".to_string(),
    }
}

pub fn fields_for_variant<'a>(resource: &'a ResourceState, variant: FieldVariant) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource.fields.iter().filter(|(_, f)| f.variants.contains(&variant)).collect()
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
    Password,
    Email,
    Url,
    Number,
    Datetime,
    Date,
    Bool,
    Textarea,
}

pub fn classify_input(field: &FieldState) -> InputKind {
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

pub fn render_create_form(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let component_name = format!("{stem}CreateForm");
    let insertable_type = format!("{stem}Insertable");

    let insertable_fields: Vec<(&FieldName, &FieldState)> = fields_for_variant(resource, FieldVariant::Insertable).into_iter().filter(|(_, f)| !f.primary_key).collect();

    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::ev::SubmitEvent;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{insertable_type};\n"));
    out.push_str(&format!("use crate::structs::generated::validators::{table}::validate_{table}_insertable;\n"));
    out.push_str("use crate::transport::leptos::components::ErrorBanner;\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::do_{table}_create;\n"));
    out.push_str("use thaw::{Checkbox, Input, InputType, Textarea};\n");
    out.push('\n');

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component_name}() -> impl IntoView {{\n"));

    for (name, field) in &insertable_fields {
        out.push_str(&render_signal_decl(name.as_str(), field, false));
    }
    out.push('\n');

    out.push_str(&format!("    let create_action: Action<{insertable_type}, ::std::result::Result<crate::structs::generated::{table}::{stem}Public, crate::meltdown::MeltDown>> = Action::new(move |input: &{insertable_type}| {{\n"));
    out.push_str("        let input = input.clone();\n");
    out.push_str("        async move {\n");
    out.push_str(&format!("            validate_{table}_insertable(&input)?;\n"));
    out.push_str(&format!("            do_{table}_create(input).await\n"));
    out.push_str("        }\n");
    out.push_str("    });\n");
    out.push('\n');

    out.push_str("    let pending = create_action.pending();\n");
    out.push_str("    let value = create_action.value();\n");
    out.push('\n');

    out.push_str("    let on_submit = move |ev: SubmitEvent| {\n");
    out.push_str("        ev.prevent_default();\n");
    out.push_str(&format!("        let input = {insertable_type} {{\n"));
    for (name, field) in &insertable_fields {
        out.push_str(&render_field_assignment(name.as_str(), field, false));
    }
    out.push_str("        };\n");
    out.push_str("        create_action.dispatch(input);\n");
    out.push_str("    };\n");
    out.push('\n');

    out.push_str(&format!("    view! {{\n        <form class=\"{table}-create-form\" on:submit=on_submit>\n"));
    for (name, field) in &insertable_fields {
        out.push_str(&render_field_view(name.as_str(), field, false));
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

pub fn render_edit_form(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let stem = type_stem_for_resource(resource);
    let component_name = format!("{stem}EditForm");
    let patch_type = format!("{stem}Patch");
    let public_type = format!("{stem}Public");
    let pk_ty = pk_rust_type(resource);

    let patch_fields: Vec<(&FieldName, &FieldState)> = fields_for_variant(resource, FieldVariant::Patch).into_iter().filter(|(_, f)| !f.primary_key).collect();

    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::ev::SubmitEvent;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{patch_type}, {public_type}}};\n"));
    out.push_str(&format!("use crate::structs::generated::validators::{table}::validate_{table}_patch;\n"));
    out.push_str("use crate::transport::leptos::components::ErrorBanner;\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::do_{table}_update;\n"));
    out.push_str("use thaw::{Checkbox, Input, InputType, Textarea};\n");
    out.push('\n');

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component_name}(initial: {public_type}) -> impl IntoView {{\n"));

    out.push_str(&format!("    let row_id: {pk_ty} = initial.id.clone();\n"));
    for (name, field) in &patch_fields {
        out.push_str(&render_signal_decl(name.as_str(), field, true));
    }
    out.push('\n');

    out.push_str(&format!("    let update_action: Action<({pk_ty}, {patch_type}), ::std::result::Result<{public_type}, crate::meltdown::MeltDown>> = Action::new(move |args: &({pk_ty}, {patch_type})| {{\n"));
    out.push_str("        let args = args.clone();\n");
    out.push_str("        async move {\n");
    out.push_str(&format!("            validate_{table}_patch(&args.1)?;\n"));
    out.push_str(&format!("            do_{table}_update(args.0, args.1).await\n"));
    out.push_str("        }\n");
    out.push_str("    });\n");
    out.push('\n');

    out.push_str("    let pending = update_action.pending();\n");
    out.push_str("    let value = update_action.value();\n");
    out.push('\n');

    out.push_str("    let on_submit = move |ev: SubmitEvent| {\n");
    out.push_str("        ev.prevent_default();\n");
    out.push_str(&format!("        let patch = {patch_type} {{\n"));
    for (name, field) in &patch_fields {
        out.push_str(&render_field_assignment(name.as_str(), field, true));
    }
    out.push_str("        };\n");
    out.push_str("        update_action.dispatch((row_id.clone(), patch));\n");
    out.push_str("    };\n");
    out.push('\n');

    out.push_str(&format!("    view! {{\n        <form class=\"{table}-edit-form\" on:submit=on_submit>\n"));
    for (name, field) in &patch_fields {
        out.push_str(&render_field_view(name.as_str(), field, true));
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

fn render_signal_decl(name: &str, field: &FieldState, is_patch: bool) -> String {
    let kind = classify_input(field);
    match kind {
        InputKind::Bool => format!("    let {name} = RwSignal::new(false);\n"),
        InputKind::Number => format!("    let {name}: RwSignal<String> = RwSignal::new(String::new());\n"),
        _other => {
            drop(is_patch);
            format!("    let {name}: RwSignal<String> = RwSignal::new(String::new());\n")
        }
    }
}

fn render_field_assignment(name: &str, field: &FieldState, is_patch: bool) -> String {
    let kind = classify_input(field);
    let nullable = field.nullable;

    let raw_expr = match kind {
        InputKind::Bool => format!("{name}.get()"),
        InputKind::Number => render_number_parse(name, field),
        InputKind::Datetime | InputKind::Date | InputKind::TextLine | InputKind::Password | InputKind::Email | InputKind::Url | InputKind::Textarea => render_text_parse(name, field),
    };

    if is_patch {
        return format!("            {name}: Some({raw_expr}),\n");
    }

    if nullable {
        match kind {
            InputKind::Bool => format!("            {name}: Some({raw_expr}),\n"),
            InputKind::Number => format!("            {name}: Some({raw_expr}),\n"),
            _other => format!("            {name}: Some({raw_expr}),\n"),
        }
    } else {
        format!("            {name}: {raw_expr},\n")
    }
}

fn render_number_parse(name: &str, field: &FieldState) -> String {
    let lowered = field.sql_type.as_str().to_ascii_lowercase();
    let target = match lowered.as_str() {
        "int2" | "smallint" | "smallserial" => "i16",
        "int4" | "integer" | "serial" => "i32",
        "int8" | "bigint" | "bigserial" => "i64",
        "float4" | "real" => "f32",
        "float8" | "double" | "double precision" => "f64",
        _other => "i64",
    };
    format!("{name}.get().parse::<{target}>().unwrap_or_default() // allow: form-side stub; BE validator authoritative")
}

fn render_text_parse(name: &str, field: &FieldState) -> String {
    let lowered = field.sql_type.as_str().to_ascii_lowercase();
    match lowered.as_str() {
        "uuid" => format!(
            "match uuid::Uuid::parse_str(&{name}.get()) {{ Ok(v) => v, Err(_e) => uuid::Uuid::nil() }} // allow: form-side stub; BE validator authoritative"
        ),
        "json" | "jsonb" => format!(
            "match serde_json::from_str::<serde_json::Value>(&{name}.get()) {{ Ok(v) => v, Err(_e) => serde_json::Value::Null }} // allow: form-side stub; BE validator authoritative"
        ),
        "timestamptz" => format!(
            "match chrono::DateTime::parse_from_rfc3339(&{name}.get()) {{ Ok(v) => v.with_timezone(&chrono::Utc), Err(_e) => chrono::Utc::now() }} // allow: form-side stub; BE validator authoritative"
        ),
        "timestamp" => format!(
            "match chrono::NaiveDateTime::parse_from_str(&{name}.get(), \"%Y-%m-%dT%H:%M:%S\") {{ Ok(v) => v, Err(_e) => chrono::Utc::now().naive_utc() }} // allow: form-side stub; BE validator authoritative"
        ),
        "date" => format!(
            "match chrono::NaiveDate::parse_from_str(&{name}.get(), \"%Y-%m-%d\") {{ Ok(v) => v, Err(_e) => chrono::Utc::now().naive_utc().date() }} // allow: form-side stub; BE validator authoritative"
        ),
        _other => format!("{name}.get()"),
    }
}

fn render_field_view(name: &str, field: &FieldState, is_patch: bool) -> String {
    drop(is_patch);
    let kind = classify_input(field);
    let label_text = pretty_label(name);
    let mut out = String::new();
    out.push_str("            <label>\n");
    out.push_str(&format!("                <span>\"{label_text}\"</span>\n"));

    match kind {
        InputKind::Bool => {
            out.push_str(&format!("                <Checkbox checked={name}/>\n"));
        }
        InputKind::Textarea => {
            out.push_str(&format!("                <Textarea value={name}/>\n"));
        }
        InputKind::Password => {
            out.push_str(&format!("                <Input value={name} input_type=InputType::Password/>\n"));
        }
        InputKind::Email => {
            out.push_str(&format!("                <Input value={name} input_type=InputType::Email/>\n"));
        }
        InputKind::Url => {
            out.push_str(&format!("                <Input value={name} input_type=InputType::Url/>\n"));
        }
        InputKind::Datetime => {
            out.push_str(&format!("                <Input value={name} input_type=InputType::DatetimeLocal/>\n"));
        }
        InputKind::Date => {
            out.push_str(&format!("                <Input value={name} input_type=InputType::Date/>\n"));
        }
        InputKind::Number => {
            out.push_str(&format!("                <Input value={name} input_type=InputType::Text/>\n"));
        }
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
    out.push_str(&format!("use crate::structs::generated::{table}::{{{insertable_type}, {patch_type}, {public_type}}};\n"));
    out.push_str("use crate::meltdown::{MeltDown, MeltType};\n");
    out.push('\n');

    if resource.verbs.contains_key(&Verb::Create) {
        out.push_str(&format!("pub async fn do_{table}_create(input: {insertable_type}) -> ::std::result::Result<{public_type}, MeltDown> {{\n"));
        out.push_str("    drop(input);\n");
        out.push_str(&format!("    Err(MeltDown::new(MeltType::Unexpected(\"not_implemented\".to_string()), \"do_{table}_create not yet implemented\"))\n"));
        out.push_str("}\n");
        out.push('\n');
    }

    if resource.verbs.contains_key(&Verb::Update) && primary_key_field(resource).is_some() {
        out.push_str(&format!("pub async fn do_{table}_update(id: {pk_ty}, patch: {patch_type}) -> ::std::result::Result<{public_type}, MeltDown> {{\n"));
        out.push_str("    drop(id);\n");
        out.push_str("    drop(patch);\n");
        out.push_str(&format!("    Err(MeltDown::new(MeltType::Unexpected(\"not_implemented\".to_string()), \"do_{table}_update not yet implemented\"))\n"));
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
