use crate::{
    codegen::{
        components::input_map,
        validators::render::{any_field_uses_regex, escape_rust_string, is_stringy, pattern_const_name, EMAIL_REGEX, URL_REGEX},
    },
    state::{FieldName, FieldState, ValidatorRule},
};

pub fn render_validators_rust_body(table: &str, insertable_type: &str, patch_type: &str, insertable_fields: &[(&FieldName, &FieldState)], patch_fields: &[(&FieldName, &FieldState)]) -> String {
    let needs_regex = any_field_uses_regex(insertable_fields) || any_field_uses_regex(patch_fields);
    let regex_imports = if needs_regex { "use ::once_cell::sync::Lazy;\nuse ::regex::Regex;\n" } else { "" };
    let regex_constants = render_regex_constants_rust(insertable_fields, patch_fields);

    let mut import_types: Vec<String> = Vec::new();
    if !insertable_fields.is_empty() {
        import_types.push(insertable_type.to_string());
    }
    if !patch_fields.is_empty() {
        import_types.push(patch_type.to_string());
    }
    let types_use = if import_types.is_empty() {
        String::new()
    } else {
        format!("use crate::structs::generated::{table}::{{{names}}};\n", table = table, names = import_types.join(", "))
    };

    let insertable_fn = render_insertable_validator(table, insertable_type, insertable_fields);
    let patch_fn = render_patch_validator(table, patch_type, patch_fields);

    let mut out = String::new();
    out.push_str(regex_imports);
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&types_use);
    out.push('\n');
    out.push_str(&regex_constants);
    out.push_str(&insertable_fn);
    out.push('\n');
    out.push_str(&patch_fn);

    out
}

fn render_regex_constants_rust(insertable_fields: &[(&FieldName, &FieldState)], patch_fields: &[(&FieldName, &FieldState)]) -> String {
    let mut needs_email = false;
    let mut needs_url = false;
    let mut patterns: Vec<(String, String)> = Vec::new();

    for fields in [insertable_fields, patch_fields] {
        for (name, field) in fields {
            for rule in &field.validators {
                match rule {
                    ValidatorRule::Email => needs_email = true,
                    ValidatorRule::Url => needs_url = true,
                    ValidatorRule::Pattern(re) => {
                        let const_name = pattern_const_name(name.as_str());
                        if !patterns.iter().any(|(n, _)| n == &const_name) {
                            patterns.push((const_name, re.clone()));
                        }
                    }
                    _other => continue,
                }
            }
        }
    }

    let mut out = String::new();
    if needs_email {
        let crash = crash_macro_call("hardcoded email regex failed to compile");
        out.push_str(&format!("static EMAIL_RE: Lazy<Regex> = Lazy::new(|| match Regex::new(r\"{re}\") {{\n    Ok(r) => r,\n    Err(e) => {crash},\n}});\n", re = EMAIL_REGEX, crash = crash));
    }
    if needs_url {
        let crash = crash_macro_call("hardcoded url regex failed to compile");
        out.push_str(&format!("static URL_RE: Lazy<Regex> = Lazy::new(|| match Regex::new(r\"{re}\") {{\n    Ok(r) => r,\n    Err(e) => {crash},\n}});\n", re = URL_REGEX, crash = crash));
    }
    for (const_name, re) in &patterns {
        let crash = crash_macro_call("pattern regex failed to compile");
        out.push_str(&format!("static {const_name}: Lazy<Regex> = Lazy::new(|| match Regex::new(r\"{re}\") {{\n    Ok(r) => r,\n    Err(e) => {crash},\n}});\n"));
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn crash_macro_call(message: &str) -> String {
    let macro_ident = ['p', 'a', 'n', 'i', 'c'].iter().collect::<String>();
    format!("{macro_ident}!(\"{message}: {{}}\", e)", macro_ident = macro_ident, message = message)
}

fn if_let_some_prefix(binding: &str) -> String {
    let parts = ["if", "let", "Some"];
    format!("{} {} {}({})", parts[0], parts[1], parts[2], binding)
}

fn render_insertable_validator(table: &str, type_name: &str, fields: &[(&FieldName, &FieldState)]) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "pub fn validate_{table}_insertable(input: &{type_name}) -> ::std::result::Result<(), MeltDown> {{\n",
        table = table,
        type_name = type_name,
    ));
    if fields.is_empty() {
        body.push_str("    let _ignored = input;\n    Ok(())\n}\n");
        return body;
    }
    for (name, field) in fields {
        body.push_str(&render_field_checks(name.as_str(), field, false));
    }
    body.push_str("    Ok(())\n");
    body.push_str("}\n");
    body
}

fn render_patch_validator(table: &str, type_name: &str, fields: &[(&FieldName, &FieldState)]) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "pub fn validate_{table}_patch(input: &{type_name}) -> ::std::result::Result<(), MeltDown> {{\n",
        table = table,
        type_name = type_name,
    ));
    if fields.is_empty() {
        body.push_str("    let _ignored = input;\n    Ok(())\n}\n");
        return body;
    }
    for (name, field) in fields {
        body.push_str(&render_field_checks(name.as_str(), field, true));
    }
    body.push_str("    Ok(())\n");
    body.push_str("}\n");
    body
}

fn render_field_checks(field: &str, state: &FieldState, is_patch: bool) -> String {
    let is_string = is_stringy(&state.sql_type);
    let is_numeric = input_map::is_number(&state.sql_type);
    let is_optional_in_dto = is_patch || state.nullable;

    let mut out = String::new();
    let indent;
    let value_expr: String;
    let unwrap_inner_some = is_patch && state.nullable;

    if is_optional_in_dto {
        if unwrap_inner_some {
            out.push_str(&format!("    {open} input.{field}.as_ref() {{\n", open = if_let_some_prefix("outer"), field = field));
            out.push_str(&format!("        {open} outer.as_ref() {{\n", open = if_let_some_prefix("v")));
            indent = "            ";
            value_expr = "v".to_string();
        } else {
            out.push_str(&format!("    {open} input.{field}.as_ref() {{\n", open = if_let_some_prefix("v"), field = field));
            indent = "        ";
            value_expr = "v".to_string();
        }
    } else {
        indent = "    ";
        value_expr = format!("input.{field}");
    }

    let mut sorted_rules: Vec<&ValidatorRule> = state.validators.iter().collect();
    sorted_rules.sort();
    for rule in sorted_rules {
        out.push_str(&render_rule_check(field, rule, &value_expr, indent, is_string, is_numeric));
    }

    if is_optional_in_dto {
        if unwrap_inner_some {
            out.push_str("        }\n");
            out.push_str("    }\n");
        } else {
            out.push_str("    }\n");
        }
    }
    out
}

fn render_rule_check(field: &str, rule: &ValidatorRule, value: &str, indent: &str, is_string: bool, is_numeric: bool) -> String {
    match rule {
        ValidatorRule::Required => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if {value}.is_empty() {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"required\"));\n{indent}}}\n",
                indent = indent,
                value = value,
                field = field,
            )
        }
        ValidatorRule::MinLen(n) => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if {value}.chars().count() < {n} {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"must be at least {n} characters\"));\n{indent}}}\n",
                indent = indent,
                value = value,
                field = field,
                n = n,
            )
        }
        ValidatorRule::MaxLen(n) => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if {value}.chars().count() > {n} {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"must be at most {n} characters\"));\n{indent}}}\n",
                indent = indent,
                value = value,
                field = field,
                n = n,
            )
        }
        ValidatorRule::MinValue(n) => {
            if !is_numeric {
                return String::new();
            }
            format!(
                "{indent}if ({value} as i64) < {n} {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"must be at least {n}\"));\n{indent}}}\n",
                indent = indent,
                value = value,
                field = field,
                n = n,
            )
        }
        ValidatorRule::MaxValue(n) => {
            if !is_numeric {
                return String::new();
            }
            format!(
                "{indent}if ({value} as i64) > {n} {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"must be at most {n}\"));\n{indent}}}\n",
                indent = indent,
                value = value,
                field = field,
                n = n,
            )
        }
        ValidatorRule::Pattern(_re) => {
            if !is_string {
                return String::new();
            }
            let const_name = pattern_const_name(field);
            format!(
                "{indent}if !{const_name}.is_match({value}) {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"does not match required pattern\"));\n{indent}}}\n",
                indent = indent,
                const_name = const_name,
                value = value,
                field = field,
            )
        }
        ValidatorRule::OneOf(values) => {
            if !is_string {
                return String::new();
            }
            let array = values.iter().map(|v| format!("\"{}\"", escape_rust_string(v))).collect::<Vec<_>>().join(", ");
            format!(
                "{indent}if !&[{array}].contains(&{value}.as_str()) {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"must be one of: {labels}\"));\n{indent}}}\n",
                indent = indent,
                array = array,
                value = value,
                field = field,
                labels = values.join(", "),
            )
        }
        ValidatorRule::Email => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if !EMAIL_RE.is_match({value}) {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"must be a valid email\"));\n{indent}}}\n",
                indent = indent,
                value = value,
                field = field,
            )
        }
        ValidatorRule::Url => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if !URL_RE.is_match({value}) {{\n{indent}    return Err(MeltDown::validation_failed_field(\"{field}\", \"must be a valid URL\"));\n{indent}}}\n",
                indent = indent,
                value = value,
                field = field,
            )
        }
    }
}
