//! Field-type → PrimeVue input-component mapping.
//!
//! Mirrors the `frontend_types::ts_base_type` catalogue but emits a
//! component name (e.g. `"InputText"`) rather than a TypeScript type.
//!
//! Per `SPEC_CODEGEN.md`:
//!
//! | SQL family             | Input component         |
//! |------------------------|-------------------------|
//! | text/varchar/char/uuid | InputText               |
//! | bool                   | Checkbox                |
//! | int*/float*/numeric    | InputNumber             |
//! | timestamp/timestamptz  | Calendar (with showTime)|
//! | date                   | Calendar                |
//! | time                   | Calendar (timeOnly)     |
//! | json/jsonb             | Textarea                |
//! | unknown                | InputText (fallback)    |
//!
//! `enum` and FK reference inputs (`Dropdown`, `AutoComplete`) are
//! resource-state driven, not SQL-type driven — handled by a separate
//! resolver in `render.rs` once the schema carries that information. For
//! now plain SQL types route through this catalogue.

use crate::state::SqlType;

/// PrimeVue component name for a given SQL type.
pub fn primevue_component(sql: &SqlType) -> &'static str {
    let lowered = sql.as_str().to_ascii_lowercase();
    match lowered.as_str() {
        "bool" | "boolean" => "Checkbox",
        "int2" | "smallint" | "smallserial" | "int4" | "integer" | "serial" | "int8" | "bigint" | "bigserial" | "float4" | "real" | "float8" | "double" | "double precision" | "numeric" | "decimal" => "InputNumber",
        "timestamp" | "timestamptz" | "date" | "time" => "Calendar",
        "json" | "jsonb" => "Textarea",
        _other => "InputText",
    }
}

/// Whether the input element is bound through `v-model` directly, or via
/// a typed wrapper (e.g. `Calendar` produces a `Date` object that we
/// serialize on submit). Used by the renderer to pick the correct
/// binding pattern.
pub fn is_calendar(sql: &SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "timestamp" | "timestamptz" | "date" | "time")
}

/// True when the field needs `showTime` on the Calendar.
pub fn calendar_show_time(sql: &SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "timestamp" | "timestamptz" | "time")
}

/// True when the field needs `timeOnly` on the Calendar.
pub fn calendar_time_only(sql: &SqlType) -> bool {
    sql.as_str().eq_ignore_ascii_case("time")
}

/// True when the underlying TS value is a number (drives `:useGrouping="false"` etc.).
pub fn is_number(sql: &SqlType) -> bool {
    matches!(
        sql.as_str().to_ascii_lowercase().as_str(),
        "int2" | "smallint" | "smallserial" | "int4" | "integer" | "serial" | "int8" | "bigint" | "bigserial" | "float4" | "real" | "float8" | "double" | "double precision" | "numeric" | "decimal"
    )
}

/// True when the field is a boolean (drives Checkbox `:binary="true"`).
pub fn is_bool(sql: &SqlType) -> bool {
    matches!(sql.as_str().to_ascii_lowercase().as_str(), "bool" | "boolean")
}

/// True when the field is JSON-shaped (drives a `<Textarea>` rendered as raw JSON).
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
}
