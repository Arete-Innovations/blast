//! Diesel SQL-type → Rust-type mapping for struct codegen.
//!
//! `FieldState.sql_type` carries a Diesel-style identifier sourced from
//! `schema.rs` (e.g. `"Int4"`, `"Varchar"`, `"Timestamptz"`, `"Bool"`).
//! The TUI wizards normalize the Diesel column type straight into a
//! `SqlType` value, so this mapper consumes whatever the diesel macro
//! emits — no SQL DDL parsing required here.
//!
//! Unknown types degrade to `String`. That keeps `cargo check` green even
//! when a project uses a custom domain or a Diesel type we haven't taught
//! Blast about yet — the user can swap in the right alias by hand and the
//! marker will refuse to drift.

use crate::state::SqlType;

/// Map a Diesel SQL type to its non-nullable Rust type representation.
///
/// Comparison is case-insensitive: state files written by hand may carry
/// `"INT8"` while the wizard emits `"Int8"`. Both should resolve to
/// `i64`.
pub fn rust_base_type(sql: &SqlType) -> &'static str {
    let lowered = sql.as_str().to_ascii_lowercase();
    match lowered.as_str() {
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
        // Unknown / not-yet-mapped types fall back to a String stand-in.
        // The user app is expected to swap in the right alias by hand,
        // and the per-resource hash marker prevents this stub from
        // silently going stale.
        _other => "String",
    }
}

/// Wrap the base Rust type in `Option<...>` when the column is nullable
/// or the projection variant demands it (e.g. `Patch` always optional).
pub fn rust_type(sql: &SqlType, nullable: bool) -> String {
    let base = rust_base_type(sql);
    match nullable {
        true => format!("Option<{base}>"),
        false => base.to_string(),
    }
}

/// Force-wrap the base type in `Option<...>` regardless of nullability.
/// Used by the `Patch` projection where every field is partial.
pub fn rust_type_always_optional(sql: &SqlType) -> String {
    let base = rust_base_type(sql);
    format!("Option<{base}>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_integer_types() {
        assert_eq!(rust_base_type(&SqlType::new("Int2")), "i16");
        assert_eq!(rust_base_type(&SqlType::new("Int4")), "i32");
        assert_eq!(rust_base_type(&SqlType::new("Int8")), "i64");
        assert_eq!(rust_base_type(&SqlType::new("BIGSERIAL")), "i64");
        assert_eq!(rust_base_type(&SqlType::new("smallint")), "i16");
    }

    #[test]
    fn maps_text_types_to_string() {
        assert_eq!(rust_base_type(&SqlType::new("Varchar")), "String");
        assert_eq!(rust_base_type(&SqlType::new("Text")), "String");
        assert_eq!(rust_base_type(&SqlType::new("CITEXT")), "String");
    }

    #[test]
    fn maps_temporal_types() {
        assert_eq!(rust_base_type(&SqlType::new("Timestamptz")), "chrono::DateTime<chrono::Utc>");
        assert_eq!(rust_base_type(&SqlType::new("Timestamp")), "chrono::NaiveDateTime");
        assert_eq!(rust_base_type(&SqlType::new("Date")), "chrono::NaiveDate");
        assert_eq!(rust_base_type(&SqlType::new("Time")), "chrono::NaiveTime");
    }

    #[test]
    fn maps_misc_types() {
        assert_eq!(rust_base_type(&SqlType::new("Bool")), "bool");
        assert_eq!(rust_base_type(&SqlType::new("Bytea")), "Vec<u8>");
        assert_eq!(rust_base_type(&SqlType::new("Uuid")), "uuid::Uuid");
        assert_eq!(rust_base_type(&SqlType::new("Jsonb")), "serde_json::Value");
        assert_eq!(rust_base_type(&SqlType::new("Json")), "serde_json::Value");
    }

    #[test]
    fn unknown_type_falls_back_to_string() {
        assert_eq!(rust_base_type(&SqlType::new("totally_made_up")), "String");
    }

    #[test]
    fn rust_type_wraps_option_when_nullable() {
        let s = SqlType::new("Int4");
        assert_eq!(rust_type(&s, false), "i32");
        assert_eq!(rust_type(&s, true), "Option<i32>");
    }

    #[test]
    fn rust_type_always_optional_wraps_option() {
        let s = SqlType::new("Varchar");
        assert_eq!(rust_type_always_optional(&s), "Option<String>");
        let nullable = SqlType::new("Int8");
        assert_eq!(rust_type_always_optional(&nullable), "Option<i64>");
    }
}
