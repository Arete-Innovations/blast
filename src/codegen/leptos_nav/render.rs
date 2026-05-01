//! Renderer for the `<AppNav/>` Leptos component.
//!
//! Reads the `NavConfig` policy out of `app.ron` and a `route_table` keyed
//! by route name (built upstream from handwritten routes + `Page`
//! entries + per-resource CRUD routes) and emits one Rust source file at
//! `src/transport/leptos/components/generated/nav/app_nav.rs`.
//!
//! Output shape:
//!
//! - one top-level `<nav>` containing one `<section>` per `NavSection`
//! - each `<a href="…">` link inside is wrapped in a `<Show when=…>`
//!   block when the entry carries `roles`
//! - the whole `<section>` is wrapped in `<Show>` when the `Section`
//!   itself carries `roles`
//! - role checks call the SessionStore has_role helper against the
//!   canonical Role enum living in src/structs/auth/role.rs. Variants in
//!   app.ron are mapped to canonical ones at codegen time: User maps to
//!   Member; Admin maps to Admin. Any future drift between state role
//!   names and canonical role names is reconciled in this module.

use std::collections::BTreeMap;

use crate::{
    codegen::leptos_nav::runner::ResolvedRoute,
    state::app::{Entry, NavConfig, Role, Section},
};

/// Map a state-side `Role` variant (the RON-level Role, see
/// `state::app::Role`) to the canonical Role enum variant name as
/// authored in `crate::structs::auth::role::Role` inside the scaffolded
/// app. The mapping is intentionally explicit — it is the only place
/// that knows the two enums are not 1:1.
fn canonical_role_variant(role: &Role) -> &'static str {
    match role {
        Role::Admin => "Admin",
        Role::User => "Member",
    }
}

/// True when the section itself OR any of its entries declares a
/// non-empty `roles: Some([...])` list — i.e. the rendered output will
/// need to import `Role` and `use_session` for at least one
/// `<Show when=...>` predicate.
fn section_or_entry_has_roles(section: &Section) -> bool {
    if non_empty_roles(section.roles.as_ref()) {
        return true;
    }
    section.entries.iter().any(|e| non_empty_roles(e.roles.as_ref()))
}

fn non_empty_roles(roles: Option<&Vec<Role>>) -> bool {
    match roles {
        Some(list) => !list.is_empty(),
        None => false, // allow: absent role list = "no gating", which is the documented zero-state
    }
}

pub fn render_app_nav(nav: &NavConfig, route_table: &BTreeMap<String, ResolvedRoute>) -> String {
    let needs_role_gating = nav.sections.iter().any(|s| section_or_entry_has_roles(s));

    let mut out = String::new();
    out.push_str("use ::leptos::prelude::*;\n\n");
    if needs_role_gating {
        out.push_str("use crate::structs::auth::role::Role;\n");
        out.push_str("use crate::transport::leptos::signals::session::use_session;\n\n");
    }
    out.push_str("#[component]\n");
    out.push_str("pub fn AppNav() -> impl IntoView {\n");
    if needs_role_gating {
        out.push_str("    let session = use_session();\n");
    }
    out.push_str("    view! {\n");
    out.push_str("        <nav class=\"app-nav\">\n");
    for section in &nav.sections {
        push_section(&mut out, section, route_table);
    }
    out.push_str("        </nav>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn push_section(out: &mut String, section: &Section, route_table: &BTreeMap<String, ResolvedRoute>) {
    let section_roles: &[Role] = match section.roles.as_deref() {
        Some(list) => list,
        None => &[], // allow: absent role list = "no gating"
    };
    let needs_section_show = !section_roles.is_empty();
    if needs_section_show {
        push_show_open(out, section_roles, 12);
    }

    let indent = if needs_section_show { 16 } else { 12 };
    push_indent(out, indent);
    out.push_str(&format!(
        "<section class=\"app-nav__section\" data-section-key={section_key}>\n",
        section_key = lit(&section.key),
    ));
    push_indent(out, indent + 4);
    out.push_str(&format!("<h2 class=\"app-nav__heading\">{label}</h2>\n", label = lit(&section.label)));

    push_indent(out, indent + 4);
    out.push_str("<ul class=\"app-nav__list\">\n");
    for entry in &section.entries {
        push_entry(out, entry, route_table, indent + 8);
    }
    push_indent(out, indent + 4);
    out.push_str("</ul>\n");

    push_indent(out, indent);
    out.push_str("</section>\n");

    if needs_section_show {
        push_show_close(out, 12);
    }
}

fn push_entry(out: &mut String, entry: &Entry, route_table: &BTreeMap<String, ResolvedRoute>, indent: usize) {
    let path = match route_table.get(&entry.route) {
        Some(resolved) => resolved.path.clone(),
        None => format!("/{}", entry.route),
    };
    let label = match entry.label.as_ref() {
        Some(value) => value.clone(),
        None => entry.route.clone(),
    };

    let entry_roles: &[Role] = match entry.roles.as_deref() {
        Some(list) => list,
        None => &[], // allow: absent role list = "no gating"
    };
    let needs_entry_show = !entry_roles.is_empty();

    if needs_entry_show {
        push_show_open(out, entry_roles, indent);
    }

    let li_indent = if needs_entry_show { indent + 4 } else { indent };
    push_indent(out, li_indent);
    out.push_str(&format!(
        "<li class=\"app-nav__item\"><a class=\"app-nav__link\" href={path_lit}>{label}</a></li>\n",
        path_lit = lit(&path),
        label = lit(&label),
    ));

    if needs_entry_show {
        push_show_close(out, indent);
    }
}

fn push_show_open(out: &mut String, roles: &[Role], indent: usize) {
    push_indent(out, indent);
    let when = role_check_expr(roles);
    out.push_str(&format!("<Show when=move || {{ {when} }}>\n"));
}

fn push_show_close(out: &mut String, indent: usize) {
    push_indent(out, indent);
    out.push_str("</Show>\n");
}

fn role_check_expr(roles: &[Role]) -> String {
    let parts: Vec<String> = roles
        .iter()
        .map(|r| {
            let variant = canonical_role_variant(r);
            format!("session.has_role(Role::{variant})")
        })
        .collect();
    if parts.is_empty() {
        return "true".to_string();
    }
    parts.join(" || ")
}

fn push_indent(out: &mut String, count: usize) {
    for _ in 0..count {
        out.push(' ');
    }
}

/// Format a string as a Leptos view! attribute / text literal — wrap in
/// double quotes and escape any embedded quotes/backslashes.
fn lit(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        codegen::leptos_nav::runner::ResolvedRoute,
        state::app::{Entry, NavConfig, Role, Section},
    };

    fn route_table() -> BTreeMap<String, ResolvedRoute> {
        let mut table = BTreeMap::new();
        table.insert("dashboard".to_string(), ResolvedRoute { path: "/dashboard".to_string() });
        table.insert("profile".to_string(), ResolvedRoute { path: "/profile".to_string() });
        table.insert("posts.list".to_string(), ResolvedRoute { path: "/posts".to_string() });
        table
    }

    fn nav(sections: Vec<Section>) -> NavConfig {
        NavConfig { sections }
    }

    #[test]
    fn empty_nav_emits_skeleton() {
        let body = render_app_nav(&nav(Vec::new()), &route_table());
        assert!(body.contains("pub fn AppNav"));
        assert!(body.contains("<nav class=\"app-nav\">"));
        assert!(body.contains("</nav>"));
    }

    #[test]
    fn entry_uses_route_table_path() {
        let cfg = nav(vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "dashboard".to_string(),
                label: Some("Dashboard".to_string()),
                icon: None,
                roles: None,
            }],
        }]);
        let body = render_app_nav(&cfg, &route_table());
        assert!(body.contains("href=\"/dashboard\""), "must reference resolved path: {body}");
        assert!(body.contains("\"Dashboard\""), "must include label: {body}");
    }

    #[test]
    fn role_gated_section_wraps_in_show() {
        let cfg = nav(vec![Section {
            key: "ops".to_string(),
            label: "Ops".to_string(),
            icon: "tools".to_string(),
            roles: Some(vec![Role::Admin]),
            entries: vec![Entry {
                route: "dashboard".to_string(),
                label: None,
                icon: None,
                roles: None,
            }],
        }]);
        let body = render_app_nav(&cfg, &route_table());
        assert!(body.contains("<Show"), "section role gating must emit <Show: {body}");
        assert!(body.contains("Role::Admin"), "must reference Role::Admin: {body}");
        assert!(body.contains("session.has_role"), "must call has_role: {body}");
    }

    #[test]
    fn role_gated_entry_wraps_in_show() {
        let cfg = nav(vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "dashboard".to_string(),
                label: None,
                icon: None,
                roles: Some(vec![Role::User]),
            }],
        }]);
        let body = render_app_nav(&cfg, &route_table());
        assert!(body.contains("Role::Member"), "User role must map to canonical Role::Member: {body}");
        assert!(body.contains("<Show"), "entry role gating must emit <Show: {body}");
    }

    #[test]
    fn label_falls_back_to_route_name() {
        let cfg = nav(vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "dashboard".to_string(),
                label: None,
                icon: None,
                roles: None,
            }],
        }]);
        let body = render_app_nav(&cfg, &route_table());
        assert!(body.contains("\"dashboard\""));
    }

    #[test]
    fn multiple_roles_emit_or_check() {
        let cfg = nav(vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "dashboard".to_string(),
                label: None,
                icon: None,
                roles: Some(vec![Role::Admin, Role::User]),
            }],
        }]);
        let body = render_app_nav(&cfg, &route_table());
        assert!(body.contains(" || "), "multi-role must emit OR-chain: {body}");
        assert!(body.contains("Role::Admin"));
        assert!(body.contains("Role::Member"));
    }

    #[test]
    fn unknown_route_falls_back_to_literal_path() {
        let mut empty_table: BTreeMap<String, ResolvedRoute> = BTreeMap::new();
        empty_table.insert("dashboard".to_string(), ResolvedRoute { path: "/dashboard".to_string() });
        let cfg = nav(vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "uncharted".to_string(),
                label: None,
                icon: None,
                roles: None,
            }],
        }]);
        let body = render_app_nav(&cfg, &empty_table);
        // Validation lives in runner.rs; render is defensive — falls back to /<route>.
        assert!(body.contains("href=\"/uncharted\""), "must fall back to literal path: {body}");
    }
}
