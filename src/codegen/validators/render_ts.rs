use crate::{
    codegen::{
        components::input_map,
        validators::render::{escape_ts_single_quote, is_stringy, pattern_const_name, EMAIL_REGEX, URL_REGEX},
    },
    state::{FieldName, FieldState, ValidatorRule},
};

pub fn render_validators_ts_body(table: &str, stem: &str, insertable_type: &str, patch_type: &str, insertable_fields: &[(&FieldName, &FieldState)], patch_fields: &[(&FieldName, &FieldState)]) -> String {
    let mut import_types: Vec<String> = Vec::new();
    if !insertable_fields.is_empty() {
        import_types.push(insertable_type.to_string());
    }
    if !patch_fields.is_empty() {
        import_types.push(patch_type.to_string());
    }

    let mut out = String::new();
    if !import_types.is_empty() {
        out.push_str(&format!(
            "import type {{ {names} }} from '@/generated/types/{table}'\n",
            names = import_types.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(", "),
            table = table,
        ));
    }
    out.push('\n');
    out.push_str("export type FieldErrors = Record<string, string>\n\n");

    let regex_constants = render_regex_constants_ts(insertable_fields, patch_fields);
    out.push_str(&regex_constants);

    out.push_str(&render_insertable_validator(stem, insertable_type, insertable_fields));
    out.push('\n');
    out.push_str(&render_patch_validator(stem, patch_type, patch_fields));

    out
}

fn render_regex_constants_ts(insertable_fields: &[(&FieldName, &FieldState)], patch_fields: &[(&FieldName, &FieldState)]) -> String {
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
        out.push_str(&format!("const EMAIL_RE = /{re}/\n", re = EMAIL_REGEX));
    }
    if needs_url {
        out.push_str(&format!("const URL_RE = /{re}/\n", re = URL_REGEX));
    }
    for (const_name, re) in &patterns {
        out.push_str(&format!("const {const_name} = /{re}/\n"));
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn render_insertable_validator(stem: &str, type_name: &str, fields: &[(&FieldName, &FieldState)]) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "export function validate{stem}Insertable(input: {type_name}): FieldErrors | null {{\n",
        stem = stem,
        type_name = type_name,
    ));
    body.push_str("  const errors: FieldErrors = {}\n");
    if fields.is_empty() {
        body.push_str("  void input\n");
        body.push_str("  return Object.keys(errors).length === 0 ? null : errors\n");
        body.push_str("}\n");
        return body;
    }
    for (name, field) in fields {
        body.push_str(&render_field_checks(name.as_str(), field, false));
    }
    body.push_str("  return Object.keys(errors).length === 0 ? null : errors\n");
    body.push_str("}\n");
    body
}

fn render_patch_validator(stem: &str, type_name: &str, fields: &[(&FieldName, &FieldState)]) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "export function validate{stem}Patch(input: {type_name}): FieldErrors | null {{\n",
        stem = stem,
        type_name = type_name,
    ));
    body.push_str("  const errors: FieldErrors = {}\n");
    if fields.is_empty() {
        body.push_str("  void input\n");
        body.push_str("  return Object.keys(errors).length === 0 ? null : errors\n");
        body.push_str("}\n");
        return body;
    }
    for (name, field) in fields {
        body.push_str(&render_field_checks(name.as_str(), field, true));
    }
    body.push_str("  return Object.keys(errors).length === 0 ? null : errors\n");
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
    if is_optional_in_dto {
        out.push_str(&format!("  if (input.{field} !== undefined && input.{field} !== null) {{\n", field = field));
        indent = "    ";
        value_expr = format!("input.{field}");
    } else {
        indent = "  ";
        value_expr = format!("input.{field}");
    }

    let mut sorted_rules: Vec<&ValidatorRule> = state.validators.iter().collect();
    sorted_rules.sort();
    for rule in sorted_rules {
        out.push_str(&render_rule_check(field, rule, &value_expr, indent, is_string, is_numeric));
    }

    if is_optional_in_dto {
        out.push_str("  }\n");
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
                "{indent}if (errors.{field} === undefined && (typeof {value} !== 'string' || {value}.length === 0)) {{\n{indent}  errors.{field} = 'required'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
            )
        }
        ValidatorRule::MinLen(n) => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'string' && [...{value}].length < {n}) {{\n{indent}  errors.{field} = 'must be at least {n} characters'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
                n = n,
            )
        }
        ValidatorRule::MaxLen(n) => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'string' && [...{value}].length > {n}) {{\n{indent}  errors.{field} = 'must be at most {n} characters'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
                n = n,
            )
        }
        ValidatorRule::MinValue(n) => {
            if !is_numeric {
                return String::new();
            }
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'number' && {value} < {n}) {{\n{indent}  errors.{field} = 'must be at least {n}'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
                n = n,
            )
        }
        ValidatorRule::MaxValue(n) => {
            if !is_numeric {
                return String::new();
            }
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'number' && {value} > {n}) {{\n{indent}  errors.{field} = 'must be at most {n}'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
                n = n,
            )
        }
        ValidatorRule::Pattern(_re) => {
            if !is_string {
                return String::new();
            }
            let const_name = pattern_const_name(field);
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'string' && !{const_name}.test({value})) {{\n{indent}  errors.{field} = 'does not match required pattern'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
                const_name = const_name,
            )
        }
        ValidatorRule::OneOf(values) => {
            if !is_string {
                return String::new();
            }
            let array = values.iter().map(|v| format!("'{}'", escape_ts_single_quote(v))).collect::<Vec<_>>().join(", ");
            let labels = values.join(", ");
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'string' && !([{array}] as readonly string[]).includes({value})) {{\n{indent}  errors.{field} = 'must be one of: {labels}'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
                array = array,
                labels = labels,
            )
        }
        ValidatorRule::Email => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'string' && !EMAIL_RE.test({value})) {{\n{indent}  errors.{field} = 'must be a valid email'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
            )
        }
        ValidatorRule::Url => {
            if !is_string {
                return String::new();
            }
            format!(
                "{indent}if (errors.{field} === undefined && typeof {value} === 'string' && !URL_RE.test({value})) {{\n{indent}  errors.{field} = 'must be a valid URL'\n{indent}}}\n",
                indent = indent,
                field = field,
                value = value,
            )
        }
    }
}
