use crate::codegen::structs::emitter::table_row::is_display_safe;
use crate::state::{AuthMode, FieldName, FieldState, FieldVariant, ResourceState};

pub fn auth_guard_mode_str(auth: &AuthMode) -> &'static str {
    match auth {
        AuthMode::Public => "AuthGuardMode::Public",
        AuthMode::AuthRequired => "AuthGuardMode::Required",
        AuthMode::AdminOnly => "AuthGuardMode::AdminOnly",
        AuthMode::Roles(_roles) => "AuthGuardMode::AdminOnly",
        AuthMode::ScopedTo(_field) => "AuthGuardMode::Required",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CellKind {
    PkLink,
    EpochSecondsAt,
    Timestamp,
    Bool,
    Plain,
}

pub(super) fn classify_cell(name: &str, field: &FieldState) -> CellKind {
    if field.primary_key {
        return CellKind::PkLink;
    }
    let sql = field.sql_type.as_str().to_ascii_lowercase();
    let lname = name.to_ascii_lowercase();
    let when_column = lname.ends_with("_at") || lname.ends_with("_on") || lname == "created" || lname == "updated";
    match sql.as_str() {
        "bool" | "boolean" => CellKind::Bool,
        "timestamp" | "timestamptz" => CellKind::Timestamp,
        "int8" | "bigint" | "bigserial" | "int4" | "integer" | "serial" | "int2" | "smallint" | "smallserial" => match when_column {
            true => CellKind::EpochSecondsAt,
            false => CellKind::Plain,
        },
        _other => CellKind::Plain,
    }
}

pub(super) fn pretty_label(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut at_word_start = true;
    for ch in name.chars() {
        match ch {
            '_' | '-' => {
                out.push(' ');
                at_word_start = true;
            }
            other => match at_word_start {
                true => {
                    for u in other.to_uppercase() {
                        out.push(u);
                    }
                    at_word_start = false;
                }
                false => out.push(other),
            },
        }
    }
    out
}

pub(super) fn display_fields<'a>(resource: &'a ResourceState) -> Vec<(&'a FieldName, &'a FieldState)> {
    resource
        .fields
        .iter()
        .filter(|(_n, f)| f.variants.contains(&FieldVariant::Public) && is_display_safe(&f.sql_type))
        .collect()
}

pub(super) fn primary_key_field<'a>(resource: &'a ResourceState) -> Option<(&'a FieldName, &'a FieldState)> {
    resource.fields.iter().find(|(_n, f)| f.primary_key)
}

pub(super) fn breadcrumb_inline_expr(stem: &str, leaf_label: &str, parent_link_label: Option<&str>) -> String {
    let table = stem.to_ascii_lowercase();
    let mut out = String::new();
    out.push_str("vec![\n");
    out.push_str("                            BreadcrumbItem { label: \"Dashboard\".to_string(), to: Some(RouteName::Dashboard) },\n");
    match parent_link_label {
        Some(link_label) => {
            out.push_str(&format!(
                "                            BreadcrumbItem {{ label: \"{link_label}\".to_string(), to: Some(RouteName::ResourceList(\"{table}\")) }},\n"
            ));
            out.push_str(&format!(
                "                            BreadcrumbItem {{ label: \"{leaf_label}\".to_string(), to: None }},\n"
            ));
        }
        None => {
            out.push_str(&format!(
                "                            BreadcrumbItem {{ label: \"{leaf_label}\".to_string(), to: None }},\n"
            ));
        }
    }
    out.push_str("                        ]");
    out
}

pub(super) fn formatter_calls(table: &str, display: &[(&FieldName, &FieldState)], indent: &str) -> String {
    let mut out = String::new();
    for (name, field) in display {
        let col = name.as_str();
        let kind = classify_cell(col, field);
        let body = match kind {
            CellKind::PkLink => format!("|v: &Value| pk_link_cell(\"{table}\", v)"),
            CellKind::EpochSecondsAt => "|v: &Value| epoch_seconds_cell(v)".to_string(),
            CellKind::Timestamp => "|v: &Value| timestamp_cell(v)".to_string(),
            CellKind::Bool => "|v: &Value| bool_value_cell(v)".to_string(),
            CellKind::Plain => continue,
        };
        out.push_str(&format!("{indent}.formatter(\"{col}\", {body})\n"));
    }
    out
}

pub(super) fn detail_formatter_calls(table: &str, display: &[(&FieldName, &FieldState)], indent: &str) -> String {
    let mut out = String::new();
    for (name, field) in display {
        let col = name.as_str();
        let kind = classify_cell(col, field);
        let body = match kind {
            CellKind::PkLink => format!("|v: &Value| pk_link_cell(\"{table}\", v)"),
            CellKind::EpochSecondsAt => "|v: &Value| epoch_seconds_long_cell(v)".to_string(),
            CellKind::Timestamp => "|v: &Value| timestamp_long_cell(v)".to_string(),
            CellKind::Bool => "|v: &Value| bool_value_cell(v)".to_string(),
            CellKind::Plain => continue,
        };
        out.push_str(&format!("{indent}.formatter(\"{col}\", {body})\n"));
    }
    out
}

pub(super) fn cell_helpers_used(display: &[(&FieldName, &FieldState)], for_detail: bool) -> Vec<&'static str> {
    let mut helpers: Vec<&'static str> = Vec::new();
    let mut has_pk = false;
    let mut has_when = false;
    let mut has_ts = false;
    let mut has_bool = false;
    for (name, field) in display {
        match classify_cell(name.as_str(), field) {
            CellKind::PkLink => has_pk = true,
            CellKind::EpochSecondsAt => has_when = true,
            CellKind::Timestamp => has_ts = true,
            CellKind::Bool => has_bool = true,
            CellKind::Plain => {}
        }
    }
    if has_pk {
        helpers.push("pk_link_cell");
    }
    if has_when {
        helpers.push(match for_detail {
            true => "epoch_seconds_long_cell",
            false => "epoch_seconds_cell",
        });
    }
    if has_ts {
        helpers.push(match for_detail {
            true => "timestamp_long_cell",
            false => "timestamp_cell",
        });
    }
    if has_bool {
        helpers.push("bool_value_cell");
    }
    helpers
}

pub(super) fn render_id_signal_block() -> String {
    r#"    let params = leptos_router::hooks::use_params_map();
    let id_signal: Memo<i64> = Memo::new(move |_| match params.read().get("id") {
        Some(raw) => match raw.parse::<i64>() {
            Ok(n) => n,
            Err(parse_err) => {
                crate::cata_log!(Warning, format!("invalid :id route param: {}", parse_err));
                -1
            }
        },
        None => -1,
    });
"#
    .to_string()
}
