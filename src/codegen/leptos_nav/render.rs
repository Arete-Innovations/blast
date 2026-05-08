//! Renderer for the nav inventory data slice. Emits a `pub static
//! NAV_INVENTORY: &[NavSection]` to
//! `src/views/components/generated/nav/inventory.rs`. The hand-written
//! component at `src/views/components/custom/app_nav.rs` iterates the
//! slice and owns all rendering / role-gating UX choices. State-side
//! `Role` variants map to canonical `UserRole` (User → Member,
//! Admin → Admin) at codegen time.

use std::collections::BTreeMap;

use crate::{
    codegen::leptos_nav::runner::ResolvedRoute,
    state::app::{Entry, NavConfig, Role, Section},
};

fn canonical_role_variant(role: &Role) -> &'static str {
    match role {
        Role::Admin => "Admin",
        Role::User => "Member",
    }
}

fn non_empty(roles: Option<&Vec<Role>>) -> bool {
    matches!(roles, Some(list) if !list.is_empty())
}

pub fn render_inventory(nav: &NavConfig, route_table: &BTreeMap<String, ResolvedRoute>) -> String {
    let needs_role_import = nav.sections.iter().any(|s| non_empty(s.roles.as_ref()) || s.entries.iter().any(|e| non_empty(e.roles.as_ref())));
    let needs_icon_import = nav.sections.iter().any(|s| s.entries.iter().any(|e| icon_lit(e.icon.as_deref()).is_some()));

    let mut out = String::new();
    if needs_icon_import {
        out.push_str("use crate::structs::custom::icons::IconKind;\n");
    }
    if needs_role_import {
        out.push_str("use crate::structs::generated::UserRole;\n");
    }
    out.push_str("use crate::structs::vendored::leptos::{NavEntry, NavSection};\n\n");
    out.push_str("pub static NAV_INVENTORY: &[NavSection] = &[\n");
    for section in &nav.sections {
        push_section(&mut out, section, route_table);
    }
    out.push_str("];\n");
    out
}

fn push_section(out: &mut String, section: &Section, route_table: &BTreeMap<String, ResolvedRoute>) {
    out.push_str("    NavSection {\n");
    out.push_str(&format!("        key: {},\n", lit(&section.key)));
    out.push_str(&format!("        label: {},\n", lit(&section.label)));
    out.push_str(&format!("        roles: {},\n", roles_lit(section.roles.as_ref())));
    out.push_str("        entries: &[\n");
    for entry in &section.entries {
        push_entry(out, entry, route_table);
    }
    out.push_str("        ],\n");
    out.push_str("    },\n");
}

fn push_entry(out: &mut String, entry: &Entry, route_table: &BTreeMap<String, ResolvedRoute>) {
    let path = match route_table.get(&entry.route) {
        Some(r) => r.path.clone(),
        None => format!("/{}", entry.route),
    };
    let label = match entry.label.as_ref() {
        Some(value) => value.clone(),
        None => entry.route.clone(),
    };
    out.push_str("            NavEntry {\n");
    out.push_str(&format!("                path: {},\n", lit(&path)));
    out.push_str(&format!("                label: {},\n", lit(&label)));
    let icon_str = match icon_lit(entry.icon.as_deref()) {
        Some(variant) => format!("Some(IconKind::{})", variant),
        None => "None".to_string(),
    };
    out.push_str(&format!("                icon: {},\n", icon_str));
    out.push_str(&format!("                roles: {},\n", roles_lit(entry.roles.as_ref())));
    out.push_str("            },\n");
}

/// Map a state-file icon string ("home", "alert-triangle", "User") to an
/// `IconKind` variant name in PascalCase. Returns `None` if the input is
/// missing or empty — codegen emits `None` then.
fn icon_lit(raw: Option<&str>) -> Option<String> {
    let s = raw?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(trimmed.len());
    let mut upper_next = true;
    for ch in trimmed.chars() {
        match ch {
            '-' | '_' | ' ' => upper_next = true,
            c if upper_next => {
                for u in c.to_uppercase() {
                    out.push(u);
                }
                upper_next = false;
            }
            c => {
                for l in c.to_lowercase() {
                    out.push(l);
                }
            }
        }
    }
    Some(out)
}

fn roles_lit(roles: Option<&Vec<Role>>) -> String {
    match roles {
        None => "&[]".to_string(),
        Some(list) if list.is_empty() => "&[]".to_string(),
        Some(list) => {
            let parts: Vec<String> = list.iter().map(|r| format!("UserRole::{}", canonical_role_variant(r))).collect();
            format!("&[{}]", parts.join(", "))
        }
    }
}

fn lit(value: &str) -> String {
    let mut e = String::with_capacity(value.len() + 2);
    e.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => e.push_str("\\\\"),
            '"' => e.push_str("\\\""),
            other => e.push(other),
        }
    }
    e.push('"');
    e
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
    fn empty_nav_emits_empty_static() {
        let body = render_inventory(&nav(Vec::new()), &route_table());
        assert!(body.contains("pub static NAV_INVENTORY: &[NavSection] = &["));
        assert!(body.contains("];"));
        assert!(!body.contains("UserRole"), "no role imports when no roles: {body}");
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
        let body = render_inventory(&cfg, &route_table());
        assert!(body.contains("path: \"/dashboard\""), "must reference resolved path: {body}");
        assert!(body.contains("label: \"Dashboard\""), "must include label: {body}");
        assert!(body.contains("roles: &[]"), "no roles → empty slice: {body}");
    }

    #[test]
    fn role_gated_section_emits_role_slice() {
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
        let body = render_inventory(&cfg, &route_table());
        assert!(body.contains("use crate::structs::generated::UserRole;"), "must import UserRole: {body}");
        assert!(body.contains("roles: &[UserRole::Admin]"), "must reference UserRole::Admin: {body}");
    }

    #[test]
    fn role_gated_entry_maps_user_to_member() {
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
        let body = render_inventory(&cfg, &route_table());
        assert!(body.contains("UserRole::Member"), "User role must map to canonical UserRole::Member: {body}");
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
        let body = render_inventory(&cfg, &route_table());
        assert!(body.contains("label: \"dashboard\""));
    }

    #[test]
    fn multiple_roles_emit_comma_separated_slice() {
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
        let body = render_inventory(&cfg, &route_table());
        assert!(body.contains("&[UserRole::Admin, UserRole::Member]"), "must emit both variants in slice: {body}");
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
        let body = render_inventory(&cfg, &empty_table);
        assert!(body.contains("path: \"/uncharted\""), "must fall back to literal path: {body}");
    }
}
