use crate::state::SqlType;

pub fn primevue_component(sql: &SqlType) -> &'static str {
    if enum_meta(sql).is_some() {
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

pub fn enum_meta(_sql: &SqlType) -> Option<(String, Vec<String>)> {
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
    use super::*;

    #[test]
    fn maps_text_to_input_text() {
        assert_eq!(primevue_component(&SqlType::new("Varchar")), "InputText");
        assert_eq!(primevue_component(&SqlType::new("text")), "InputText");
        assert_eq!(primevue_component(&SqlType::new("Uuid")), "InputText");
    }

    #[test]
    fn maps_bool_to_checkbox() {
        assert_eq!(primevue_component(&SqlType::new("Bool")), "Checkbox");
        assert_eq!(primevue_component(&SqlType::new("BOOLEAN")), "Checkbox");
        assert!(is_bool(&SqlType::new("Bool")));
    }

    #[test]
    fn maps_numeric_to_input_number() {
        assert_eq!(primevue_component(&SqlType::new("Int4")), "InputNumber");
        assert_eq!(primevue_component(&SqlType::new("Int8")), "InputNumber");
        assert_eq!(primevue_component(&SqlType::new("Float4")), "InputNumber");
        assert_eq!(primevue_component(&SqlType::new("Numeric")), "InputNumber");
        assert!(is_number(&SqlType::new("Int8")));
    }

    #[test]
    fn maps_temporal_to_calendar() {
        assert_eq!(primevue_component(&SqlType::new("Timestamp")), "Calendar");
        assert_eq!(primevue_component(&SqlType::new("Timestamptz")), "Calendar");
        assert_eq!(primevue_component(&SqlType::new("Date")), "Calendar");
        assert_eq!(primevue_component(&SqlType::new("Time")), "Calendar");
        assert!(is_calendar(&SqlType::new("Timestamptz")));
        assert!(calendar_show_time(&SqlType::new("Timestamptz")));
        assert!(!calendar_show_time(&SqlType::new("Date")));
        assert!(calendar_time_only(&SqlType::new("Time")));
    }

    #[test]
    fn maps_json_to_textarea() {
        assert_eq!(primevue_component(&SqlType::new("Jsonb")), "Textarea");
        assert!(is_json(&SqlType::new("Json")));
    }

    #[test]
    fn unknown_falls_back_to_input_text() {
        assert_eq!(primevue_component(&SqlType::new("custom_domain")), "InputText");
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
        assert!(enum_meta(&SqlType::new("Varchar")).is_none());
        assert!(enum_meta(&SqlType::new("Int8")).is_none());
        assert!(enum_meta(&SqlType::new("Bool")).is_none());
    }
}
