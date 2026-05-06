//! Per-enum file renderer for SQL → Rust ENUM codegen.
//!
//! Produces a single `.rs` file per `CREATE TYPE x AS ENUM (...)` matching
//! the canonical hand-rolled `Role` shape: a Rust enum with `as_str` /
//! `parse` helpers and `FromSql` / `ToSql` impls bound to the codegen'd
//! `sql_types::<TypeName>` marker emitted by `diesel print-schema`.

use crate::codegen::enums::scan::{pascalize, ParsedEnum};
use crate::state::enum_meta::EnumMeta;

/// PascalCase the snake_case Postgres enum type name.
/// `user_role` -> `UserRole`, `post_status` -> `PostStatus`.
///
/// Implemented locally rather than via Inflector's `to_class_case`
/// because Inflector's class-case ALSO singularizes
/// (`status` -> `Statu`), which is wrong for ENUM names: `status` is
/// a perfectly valid Postgres type name and must round-trip unchanged.
pub fn enum_type_name(snake: &str) -> String {
    pascalize(snake)
}

/// PascalCase a single variant string. `admin` -> `Admin`,
/// `in_progress` -> `InProgress`. Same non-singularizing rule as
/// [`enum_type_name`].
pub fn variant_name(variant: &str) -> String {
    pascalize(variant)
}

/// Build the full Rust file body for one enum (without the codegen
/// marker — the runner prepends it).
///
/// Diesel imports + `AsExpression`/`FromSqlRow` derives + `FromSql`/`ToSql`
/// impls are gated `#[cfg(not(target_arch = "wasm32"))]` because diesel has no
/// wasm32 target. The bare enum + `as_str` + `parse` stay ungated so wasm
/// consumers (Leptos forms, validators) can still use the type.
///
/// The SQL type marker — also named `<TypeName>` in the schema's `sql_types`
/// module — is aliased to `<TypeName>SqlType` on import to avoid a name
/// collision with the Rust enum we're defining.
pub fn render_enum_file(parsed: &ParsedEnum, meta: &EnumMeta) -> String {
    let type_name = enum_type_name(&parsed.name);
    let sql_alias = format!("{type_name}SqlType");
    let variants: Vec<(String, String)> = parsed.variants.iter().map(|v| (variant_name(v), v.clone())).collect();

    let mut out = String::new();

    out.push_str("use serde::{Deserialize, Serialize};\n\n");
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("use std::io::Write;\n\n");
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("use diesel::backend::Backend;\n");
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("use diesel::deserialize::{self, FromSql, FromSqlRow};\n");
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("use diesel::expression::AsExpression;\n");
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("use diesel::pg::Pg;\n");
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str("use diesel::serialize::{self, IsNull, Output, ToSql};\n\n");
    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str(&format!("use crate::database::schema::sql_types::{type_name} as {sql_alias};\n"));
    out.push_str("use crate::meltdown::*;\n\n");

    out.push_str("#[cfg_attr(not(target_arch = \"wasm32\"), derive(AsExpression, FromSqlRow))]\n");
    out.push_str(&format!("#[cfg_attr(not(target_arch = \"wasm32\"), diesel(sql_type = {sql_alias}))]\n"));
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]\n");
    out.push_str(&format!("pub enum {type_name} {{\n"));
    for (rust, _sql) in &variants {
        out.push_str(&format!("    {rust},\n"));
    }
    out.push_str("}\n\n");

    out.push_str(&format!("impl {type_name} {{\n"));
    out.push_str("    pub fn as_str(&self) -> &'static str {\n");
    out.push_str("        match self {\n");
    for (rust, sql) in &variants {
        out.push_str(&format!("            {type_name}::{rust} => \"{sql}\",\n", sql = escape_str(sql)));
    }
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    out.push_str("    pub fn parse(s: &str) -> Result<Self, MeltDown> {\n");
    out.push_str("        match s {\n");
    for (rust, sql) in &variants {
        out.push_str(&format!("            \"{sql}\" => Ok({type_name}::{rust}),\n", sql = escape_str(sql)));
    }
    out.push_str(&format!("            other => Err(MeltDown::validation_failed(format!(\"unknown {snake}: {{}}\", other))),\n", snake = parsed.name));
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str(&format!("impl FromSql<{sql_alias}, Pg> for {type_name} {{\n"));
    out.push_str("    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {\n");
    out.push_str("        match bytes.as_bytes() {\n");
    for (rust, sql) in &variants {
        out.push_str(&format!("            b\"{sql}\" => Ok({type_name}::{rust}),\n", sql = escape_str(sql)));
    }
    out.push_str(&format!(
        "            other => Err(format!(\"unknown {snake}: {{}}\", String::from_utf8_lossy(other)).into()),\n",
        snake = parsed.name
    ));
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str(&format!("impl ToSql<{sql_alias}, Pg> for {type_name} {{\n"));
    out.push_str("    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {\n");
    out.push_str("        out.write_all(self.as_str().as_bytes())?;\n");
    out.push_str("        Ok(IsNull::No)\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(&format!("impl {type_name} {{\n"));
    out.push_str(&format!(
        "    pub const OPTIONS: &'static [({type_name}, &'static str, Option<&'static str>)] = &[\n"
    ));
    for (rust, sql) in &variants {
        let (label, category) = label_and_category(meta, sql, rust);
        let category_lit = match category {
            Some(c) => format!("Some(\"{}\")", escape_str(&c)),
            None => "None".to_string(),
        };
        out.push_str(&format!("        ({type_name}::{rust}, \"{label}\", {category_lit}),\n", label = escape_str(&label)));
    }
    out.push_str("    ];\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(not(target_arch = \"wasm32\"))]\n");
    out.push_str(&format!("impl crate::structs::vendored::enum_options::EnumOptions for {type_name} {{\n"));
    out.push_str("    fn options() -> &'static [(Self, &'static str, Option<&'static str>)] {\n");
    out.push_str(&format!("        {type_name}::OPTIONS\n"));
    out.push_str("    }\n");
    out.push_str("    fn as_str(&self) -> &'static str {\n");
    out.push_str(&format!("        {type_name}::as_str(self)\n"));
    out.push_str("    }\n");
    out.push_str("    fn parse(s: &str) -> Option<Self> {\n");
    out.push_str(&format!("        {type_name}::parse(s).ok()\n")); // allow: literal '.ok()' inside emitted Rust string — not a real .ok() call site
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("impl crate::structs::vendored::enum_options::EnumOptions for {type_name} {{\n"));
    out.push_str("    fn options() -> &'static [(Self, &'static str, Option<&'static str>)] {\n");
    out.push_str(&format!("        {type_name}::OPTIONS\n"));
    out.push_str("    }\n");
    out.push_str("    fn as_str(&self) -> &'static str {\n");
    out.push_str(&format!("        {type_name}::as_str(self)\n"));
    out.push_str("    }\n");
    out.push_str("    fn parse(s: &str) -> Option<Self> {\n");
    out.push_str(&format!("        {type_name}::parse(s).ok()\n")); // allow: literal '.ok()' inside emitted Rust string — not a real .ok() call site
    out.push_str("    }\n");
    out.push_str("}\n");

    out
}

fn label_and_category(meta: &EnumMeta, sql_variant: &str, pascal: &str) -> (String, Option<String>) {
    match meta.lookup(sql_variant) {
        Some(v) => (v.label.clone(), v.category.clone()),
        None => (pascal.to_string(), None),
    }
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fixture(name: &str, variants: &[&str]) -> ParsedEnum {
        ParsedEnum {
            name: name.to_string(),
            variants: variants.iter().map(|v| v.to_string()).collect(),
            source_file: PathBuf::from("/tmp/fake_up.sql"),
        }
    }

    #[test]
    fn enum_type_name_pascal_cases_snake() {
        assert_eq!(enum_type_name("user_role"), "UserRole");
        assert_eq!(enum_type_name("status"), "Status");
        assert_eq!(enum_type_name("post_publish_state"), "PostPublishState");
    }

    #[test]
    fn variant_name_pascal_cases_each_variant() {
        assert_eq!(variant_name("admin"), "Admin");
        assert_eq!(variant_name("in_progress"), "InProgress");
        assert_eq!(variant_name("OPEN"), "Open");
    }

    #[test]
    fn render_emits_expected_top_level_shape() {
        let p = fixture("user_role", &["admin", "member"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("use crate::database::schema::sql_types::UserRole as UserRoleSqlType;"));
        assert!(body.contains("use crate::meltdown::*;"));
        assert!(body.contains("diesel(sql_type = UserRoleSqlType)"));
        assert!(body.contains("pub enum UserRole {"));
        assert!(body.contains("    Admin,"));
        assert!(body.contains("    Member,"));
        assert!(body.contains("#[cfg_attr(not(target_arch = \"wasm32\"), derive(AsExpression, FromSqlRow))]"));
    }

    #[test]
    fn render_emits_as_str_match() {
        let p = fixture("user_role", &["admin", "member"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("UserRole::Admin => \"admin\""));
        assert!(body.contains("UserRole::Member => \"member\""));
    }

    #[test]
    fn render_emits_parse_match() {
        let p = fixture("user_role", &["admin", "member"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("\"admin\" => Ok(UserRole::Admin)"));
        assert!(body.contains("\"member\" => Ok(UserRole::Member)"));
        assert!(body.contains("MeltDown::validation_failed(format!(\"unknown user_role: {}\", other))"));
    }

    #[test]
    fn render_emits_from_sql_and_to_sql() {
        let p = fixture("user_role", &["admin", "member"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("impl FromSql<UserRoleSqlType, Pg> for UserRole"));
        assert!(body.contains("b\"admin\" => Ok(UserRole::Admin)"));
        assert!(body.contains("impl ToSql<UserRoleSqlType, Pg> for UserRole"));
        assert!(body.contains("out.write_all(self.as_str().as_bytes())?;"));
    }

    #[test]
    fn render_handles_multi_word_variants() {
        let p = fixture("status", &["in_progress", "done"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("Status::InProgress => \"in_progress\""));
        assert!(body.contains("\"in_progress\" => Ok(Status::InProgress)"));
        assert!(body.contains("b\"in_progress\" => Ok(Status::InProgress)"));
    }

    #[test]
    fn render_escapes_quotes_in_variant_strings() {
        let p = fixture("kind", &["a\"b", "c"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("\"a\\\"b\""));
    }

    #[test]
    fn render_round_trip_byte_stable() {
        let p = fixture("user_role", &["admin", "member"]);
        let a = render_enum_file(&p, &EnumMeta::empty(&p.name));
        let b = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert_eq!(a, b);
    }

    #[test]
    fn render_emits_options_const_with_pascal_fallback_when_no_meta() {
        let p = fixture("user_role", &["admin", "member"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("pub const OPTIONS: &'static [(UserRole, &'static str, Option<&'static str>)]"));
        assert!(body.contains("(UserRole::Admin, \"Admin\", None)"));
        assert!(body.contains("(UserRole::Member, \"Member\", None)"));
    }

    #[test]
    fn render_emits_options_with_metadata_label_and_category() {
        let p = fixture("feature_kind", &["abs", "esp"]);
        let mut meta = EnumMeta::empty(&p.name);
        meta.variants.insert(
            "abs".to_string(),
            crate::state::enum_meta::VariantMeta {
                label: "ABS".to_string(),
                category: Some("Siguranță".to_string()),
            },
        );
        meta.variants.insert(
            "esp".to_string(),
            crate::state::enum_meta::VariantMeta {
                label: "ESP".to_string(),
                category: Some("Siguranță".to_string()),
            },
        );
        let body = render_enum_file(&p, &meta);
        assert!(body.contains("(FeatureKind::Abs, \"ABS\", Some(\"Siguranță\"))"));
        assert!(body.contains("(FeatureKind::Esp, \"ESP\", Some(\"Siguranță\"))"));
    }

    #[test]
    fn render_emits_enum_options_trait_impl() {
        let p = fixture("user_role", &["admin", "member"]);
        let body = render_enum_file(&p, &EnumMeta::empty(&p.name));
        assert!(body.contains("impl crate::structs::vendored::enum_options::EnumOptions for UserRole"));
        assert!(body.contains("UserRole::OPTIONS"));
    }
}
