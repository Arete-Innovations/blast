use crate::{
    codegen::enums::{pascalize, ParsedEnum},
    state::SqlType,
};

pub fn primevue_component(sql: &SqlType, enums: &[ParsedEnum]) -> &'static str {
    if enum_meta(sql, enums).is_some() {
        return "Dropdown";
    }
    let lowered = sql.as_str().to_ascii_lowercase();
    match lowered.as_str() {
        "bool" | "boolean" => "Checkbox",
        "int2" | "smallint" | "smallserial" | "int4" | "integer" | "serial" | "int8" | "bigint" | "bigserial" | "float4" | "real" | "float8" | "double" | "double precision" | "numeric" | "decimal" => "InputNumber",
        "timestamp" | "timestamptz" | "date" | "time" => "Calendar",
        "json" | "jsonb" => "Textarea",
        _other => "InputText",
    }
}

pub fn enum_meta(sql: &SqlType, enums: &[ParsedEnum]) -> Option<(String, Vec<String>)> {
    let target = sql.as_str();
    for e in enums {
        if pascalize(&e.name) == target || e.name == target {
            return Some((e.name.clone(), e.variants.clone()));
        }
    }
    None
}

pub fn enum_type_alias(enum_name: &str) -> String {
    let mut out = String::with_capacity(enum_name.len());
    let mut upper_next = true;
    for ch in enum_name.chars() {
        if ch == '_' || ch == '-' {
            upper_next = true;
            continue;
        }
        if upper_next {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            for l in ch.to_lowercase() {
                out.push(l);
            }
        }
    }
    out
}

pub fn enum_options_const_name(enum_name: &str) -> String {
    let mut out = String::with_capacity(enum_name.len());
    for ch in enum_name.chars() {
        if ch == '-' {
            out.push('_');
            continue;
        }
        for u in ch.to_uppercase() {
            out.push(u);
        }
    }
    if out.ends_with("_VALUES") {
        out
    } else {
        format!("{out}_VALUES")
    }
}

pub fn is_calendar(sql: &SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "timestamp" | "timestamptz" | "date" | "time")
}

pub fn calendar_show_time(sql: &SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "timestamp" | "timestamptz" | "time")
}

pub fn calendar_time_only(sql: &SqlType) -> bool {
    sql.as_str().eq_ignore_ascii_case("time")
}

pub fn is_number(sql: &SqlType) -> bool {
    matches!(
        sql.as_str().to_ascii_lowercase().as_str(),
        "int2" | "smallint" | "smallserial" | "int4" | "integer" | "serial" | "int8" | "bigint" | "bigserial" | "float4" | "real" | "float8" | "double" | "double precision" | "numeric" | "decimal"
    )
}

pub fn is_bool(sql: &SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "bool" | "boolean")
}

pub fn is_json(sql: &SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "json" | "jsonb")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn enums_fixture() -> Vec<ParsedEnum> {
        vec![ParsedEnum {
            name: "user_role".to_string(),
            variants: vec!["admin".to_string(), "member".to_string()],
            source_file: PathBuf::from("/tmp/dummy.sql"),
        }]
    }

    #[test]
    fn maps_text_to_input_text() {
        let enums: Vec<ParsedEnum> = Vec::new();
        assert_eq!(primevue_component(&SqlType::new("Varchar"), &enums), "InputText");
        assert_eq!(primevue_component(&SqlType::new("text"), &enums), "InputText");
        assert_eq!(primevue_component(&SqlType::new("Uuid"), &enums), "InputText");
    }

    #[test]
    fn maps_bool_to_checkbox() {
        let enums: Vec<ParsedEnum> = Vec::new();
        assert_eq!(primevue_component(&SqlType::new("Bool"), &enums), "Checkbox");
        assert_eq!(primevue_component(&SqlType::new("BOOLEAN"), &enums), "Checkbox");
        assert!(is_bool(&SqlType::new("Bool")));
    }

    #[test]
    fn maps_numeric_to_input_number() {
        let enums: Vec<ParsedEnum> = Vec::new();
        assert_eq!(primevue_component(&SqlType::new("Int4"), &enums), "InputNumber");
        assert_eq!(primevue_component(&SqlType::new("Int8"), &enums), "InputNumber");
        assert_eq!(primevue_component(&SqlType::new("Float4"), &enums), "InputNumber");
        assert_eq!(primevue_component(&SqlType::new("Numeric"), &enums), "InputNumber");
        assert!(is_number(&SqlType::new("Int8")));
    }

    #[test]
    fn maps_temporal_to_calendar() {
        let enums: Vec<ParsedEnum> = Vec::new();
        assert_eq!(primevue_component(&SqlType::new("Timestamp"), &enums), "Calendar");
        assert_eq!(primevue_component(&SqlType::new("Timestamptz"), &enums), "Calendar");
        assert_eq!(primevue_component(&SqlType::new("Date"), &enums), "Calendar");
        assert_eq!(primevue_component(&SqlType::new("Time"), &enums), "Calendar");
        assert!(is_calendar(&SqlType::new("Timestamptz")));
        assert!(calendar_show_time(&SqlType::new("Timestamptz")));
        assert!(!calendar_show_time(&SqlType::new("Date")));
        assert!(calendar_time_only(&SqlType::new("Time")));
    }

    #[test]
    fn maps_json_to_textarea() {
        let enums: Vec<ParsedEnum> = Vec::new();
        assert_eq!(primevue_component(&SqlType::new("Jsonb"), &enums), "Textarea");
        assert!(is_json(&SqlType::new("Json")));
    }

    #[test]
    fn unknown_falls_back_to_input_text() {
        let enums: Vec<ParsedEnum> = Vec::new();
        assert_eq!(primevue_component(&SqlType::new("custom_domain"), &enums), "InputText");
    }

    #[test]
    fn enum_type_alias_pascal_cases_snake() {
        assert_eq!(enum_type_alias("user_role"), "UserRole");
        assert_eq!(enum_type_alias("task_status"), "TaskStatus");
        assert_eq!(enum_type_alias("priority"), "Priority");
    }

    #[test]
    fn enum_type_alias_handles_kebab_and_uppercase_input() {
        assert_eq!(enum_type_alias("user-role"), "UserRole");
        assert_eq!(enum_type_alias("USER_ROLE"), "UserRole");
    }

    #[test]
    fn enum_options_const_name_screams_snake() {
        assert_eq!(enum_options_const_name("user_role"), "USER_ROLE_VALUES");
        assert_eq!(enum_options_const_name("priority"), "PRIORITY_VALUES");
        assert_eq!(enum_options_const_name("task-status"), "TASK_STATUS_VALUES");
    }

    #[test]
    fn enum_meta_returns_none_for_plain_sql_types() {
        let enums: Vec<ParsedEnum> = Vec::new();
        assert!(enum_meta(&SqlType::new("Varchar"), &enums).is_none());
        assert!(enum_meta(&SqlType::new("Int8"), &enums).is_none());
        assert!(enum_meta(&SqlType::new("Bool"), &enums).is_none());
    }

    #[test]
    fn enum_meta_matches_pascalized_diesel_form() {
        let enums = enums_fixture();
        let hit = enum_meta(&SqlType::new("UserRole"), &enums).expect("match by pascalized name");
        assert_eq!(hit.0, "user_role");
        assert_eq!(hit.1, vec!["admin".to_string(), "member".to_string()]);
    }

    #[test]
    fn enum_meta_matches_raw_snake_case_form() {
        let enums = enums_fixture();
        let hit = enum_meta(&SqlType::new("user_role"), &enums).expect("match by snake_case name");
        assert_eq!(hit.0, "user_role");
    }

    #[test]
    fn enum_meta_misses_when_no_match() {
        let enums = enums_fixture();
        assert!(enum_meta(&SqlType::new("Varchar"), &enums).is_none());
        assert!(enum_meta(&SqlType::new("OtherEnum"), &enums).is_none());
    }

    #[test]
    fn primevue_component_uses_dropdown_when_enum_matches() {
        let enums = enums_fixture();
        assert_eq!(primevue_component(&SqlType::new("UserRole"), &enums), "Dropdown");
        assert_eq!(primevue_component(&SqlType::new("user_role"), &enums), "Dropdown");
    }
}
