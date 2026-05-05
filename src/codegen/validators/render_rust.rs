use crate::{
    codegen::validators::render::{escape_rust_string, is_stringy},
    state::{FieldName, FieldState, SqlType, ValidatorRule},
};

fn is_numeric_sql_type(sql: &SqlType) -> bool {
    matches!(
        sql.as_str().to_ascii_lowercase().as_str(),
        "int2" | "int4" | "int8" | "smallint" | "integer" | "bigint" | "float4" | "float8" | "real" | "double" | "numeric" | "decimal"
    )
}

pub fn render_validators_rust_body(table: &str, insertable_type: &str, patch_type: &str, insertable_fields: &[(&FieldName, &FieldState)], patch_fields: &[(&FieldName, &FieldState)], has_insertable: bool, has_patch: bool) -> String {
    let mut imported: Vec<&str> = Vec::new();
    if has_insertable {
        imported.push(insertable_type);
    }
    if has_patch {
        imported.push(patch_type);
    }
    let types_use = match imported.is_empty() {
        true => String::new(),
        false => format!("use crate::structs::generated::{table}::{{{}}};\n", imported.join(", ")),
    };

    let mut out = String::new();
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&types_use);
    out.push_str("use crate::structs::vendored::validators::{Rule, Validate};\n");
    out.push('\n');

    let const_names_insertable = render_rule_consts(table, "insertable", insertable_fields, &mut out);
    let const_names_patch = render_rule_consts(table, "patch", patch_fields, &mut out);

    if has_insertable {
        out.push_str(&render_validate_impl(insertable_type, insertable_fields, &const_names_insertable, false));
        out.push('\n');
    }
    if has_patch {
        out.push_str(&render_validate_impl(patch_type, patch_fields, &const_names_patch, true));
    }

    out
}

fn rule_const_name(table: &str, scope: &str, field: &str) -> String {
    let mut out = String::new();
    for ch in table.chars() {
        for u in ch.to_uppercase() {
            out.push(u);
        }
    }
    out.push('_');
    for ch in scope.chars() {
        for u in ch.to_uppercase() {
            out.push(u);
        }
    }
    out.push('_');
    for ch in field.chars() {
        for u in ch.to_uppercase() {
            out.push(u);
        }
    }
    out.push_str("_RULES");
    out
}

fn rule_applies_to_field(rule: &ValidatorRule, is_string: bool, is_numeric: bool) -> bool {
    match rule {
        ValidatorRule::Required | ValidatorRule::MinLen(_) | ValidatorRule::MaxLen(_) | ValidatorRule::Pattern(_) | ValidatorRule::OneOf(_) | ValidatorRule::Email | ValidatorRule::Url => is_string,
        ValidatorRule::MinValue(_) | ValidatorRule::MaxValue(_) => is_numeric,
    }
}

fn render_rule_consts(table: &str, scope: &str, fields: &[(&FieldName, &FieldState)], out: &mut String) -> Vec<(String, String)> {
    let mut const_names: Vec<(String, String)> = Vec::new();
    for (name, field) in fields {
        let is_string = is_stringy(&field.sql_type);
        let is_numeric = is_numeric_sql_type(&field.sql_type);
        let mut sorted_rules: Vec<&ValidatorRule> = field.validators.iter().filter(|r| rule_applies_to_field(r, is_string, is_numeric)).collect();
        sorted_rules.sort();
        if sorted_rules.is_empty() {
            continue;
        }
        let const_name = rule_const_name(table, scope, name.as_str());
        let rule_lits: Vec<String> = sorted_rules.iter().map(|r| render_rule_literal(r)).collect();
        out.push_str(&format!("const {const_name}: &[Rule] = &[{}];\n", rule_lits.join(", ")));
        const_names.push((name.as_str().to_string(), const_name));
    }
    if !const_names.is_empty() {
        out.push('\n');
    }
    const_names
}

fn render_rule_literal(rule: &ValidatorRule) -> String {
    match rule {
        ValidatorRule::Required => "Rule::Required".to_string(),
        ValidatorRule::MinLen(n) => format!("Rule::MinLen({n})"),
        ValidatorRule::MaxLen(n) => format!("Rule::MaxLen({n})"),
        ValidatorRule::MinValue(n) => format!("Rule::MinValue({n}.0)"),
        ValidatorRule::MaxValue(n) => format!("Rule::MaxValue({n}.0)"),
        ValidatorRule::Pattern(re) => format!("Rule::Pattern(\"{}\")", escape_rust_string(re)),
        ValidatorRule::OneOf(values) => {
            let lits: Vec<String> = values.iter().map(|v| format!("\"{}\"", escape_rust_string(v))).collect();
            format!("Rule::OneOf(&[{}])", lits.join(", "))
        }
        ValidatorRule::Email => "Rule::Email".to_string(),
        ValidatorRule::Url => "Rule::Url".to_string(),
    }
}

fn render_validate_impl(type_name: &str, fields: &[(&FieldName, &FieldState)], const_names: &[(String, String)], is_patch: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("impl Validate for {type_name} {{\n"));
    out.push_str("    fn check(&self) -> ::std::result::Result<(), MeltDown> {\n");
    if const_names.is_empty() {
        out.push_str("        Ok(())\n    }\n\n");
    } else {
        for (name, state) in fields {
            let const_name = lookup_const(name.as_str(), const_names);
            if const_name.is_empty() {
                continue;
            }
            out.push_str(&render_field_check_block(name.as_str(), state, &const_name, is_patch));
        }
        out.push_str("        Ok(())\n    }\n\n");
    }

    out.push_str("    fn rules_for(field: &str) -> &'static [Rule] {\n");
    if const_names.is_empty() {
        out.push_str("        &[]\n    }\n");
    } else {
        let mut first = true;
        for (field, const_name) in const_names {
            let kw = match first {
                true => "if",
                false => "else if",
            };
            out.push_str(&format!("        {kw} field == \"{field}\" {{ {const_name} }}\n"));
            first = false;
        }
        out.push_str("        else { &[] }\n    }\n");
    }
    out.push_str("}\n");
    out
}

fn lookup_const(field: &str, const_names: &[(String, String)]) -> String {
    for (name, c) in const_names {
        if name == field {
            return c.clone();
        }
    }
    String::new()
}

fn render_field_check_block(field: &str, state: &FieldState, const_name: &str, is_patch: bool) -> String {
    let is_string = is_stringy(&state.sql_type);
    let is_numeric = is_numeric_sql_type(&state.sql_type);
    let needs_to_string = is_numeric && !is_string;
    let is_optional_in_dto = is_patch || state.nullable;
    let unwrap_inner_some = is_patch && state.nullable;

    if is_optional_in_dto {
        let mut out = String::new();
        if unwrap_inner_some {
            out.push_str(&format!("        match self.{field}.as_ref() {{\n"));
            out.push_str("            Some(outer) => match outer.as_ref() {\n");
            out.push_str("                Some(v) => {\n");
            if needs_to_string {
                out.push_str(&format!("                    let __s = v.to_string();\n"));
                out.push_str(&format!("                    for rule in {const_name} {{ rule.check(\"{field}\", &__s)?; }}\n"));
            } else if is_string {
                out.push_str(&format!("                    for rule in {const_name} {{ rule.check(\"{field}\", v)?; }}\n"));
            } else {
                out.push_str(&format!("                    let __s = v.to_string();\n"));
                out.push_str(&format!("                    for rule in {const_name} {{ rule.check(\"{field}\", &__s)?; }}\n"));
            }
            out.push_str("                }\n");
            out.push_str("                None => {}\n");
            out.push_str("            }\n");
            out.push_str("            None => {}\n");
            out.push_str("        }\n");
        } else {
            out.push_str(&format!("        match self.{field}.as_ref() {{\n"));
            out.push_str("            Some(v) => {\n");
            if needs_to_string {
                out.push_str(&format!("                let __s = v.to_string();\n"));
                out.push_str(&format!("                for rule in {const_name} {{ rule.check(\"{field}\", &__s)?; }}\n"));
            } else if is_string {
                out.push_str(&format!("                for rule in {const_name} {{ rule.check(\"{field}\", v)?; }}\n"));
            } else {
                out.push_str(&format!("                let __s = v.to_string();\n"));
                out.push_str(&format!("                for rule in {const_name} {{ rule.check(\"{field}\", &__s)?; }}\n"));
            }
            out.push_str("            }\n");
            out.push_str("            None => {}\n");
            out.push_str("        }\n");
        }
        out
    } else {
        if is_string {
            format!("        for rule in {const_name} {{ rule.check(\"{field}\", &self.{field})?; }}\n")
        } else {
            format!("        let __s_{field} = self.{field}.to_string();\n        for rule in {const_name} {{ rule.check(\"{field}\", &__s_{field})?; }}\n")
        }
    }
}
