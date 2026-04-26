//! Small TS-emit helpers shared by routes / route-names / menu / guards.
//! No template engine, just `String`-building primitives.
//!
//! Every helper here MUST produce output that complies with Governor: no
//! `any`, no `console.log`, no `||`/`??` literal fallbacks, no `@ts-ignore`.

use crate::state::{PageLayout, Role};

use super::resolve::RouteAuth;

pub fn page_layout_literal(layout: &PageLayout) -> &'static str {
    match layout {
        PageLayout::Cards => "'cards'",
        PageLayout::Split => "'split'",
        PageLayout::Table => "'table'",
        PageLayout::Bleed => "'bleed'",
        PageLayout::Tabbed => "'tabbed'",
    }
}

pub fn role_literal(role: &Role) -> &'static str {
    match role {
        Role::User => "'user'",
        Role::Admin => "'admin'",
    }
}

/// Render a TS literal of `readonly Role[]` for the given role list.
/// Always returns `[Role.A, Role.B, ...] as const` style — never null.
pub fn role_list_literal(roles: &[Role]) -> String {
    let body: Vec<&str> = roles.iter().map(role_literal).collect();
    format!("[{}] as const", body.join(", "))
}

/// Render a TS literal of `readonly Role[] | null` — `null` for Public,
/// `[]` for Required (any signed-in user), the literal list for Roles.
pub fn auth_roles_literal(auth: &RouteAuth) -> String {
    match auth {
        RouteAuth::Public => "null".to_string(),
        RouteAuth::Required => "[] as const".to_string(),
        RouteAuth::Roles(roles) => role_list_literal(roles),
    }
}

/// Single-quote-escape a TS string literal. Backslash and single quotes
/// only — we never emit user-controlled multiline content here.
pub fn ts_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{}'", escaped)
}
