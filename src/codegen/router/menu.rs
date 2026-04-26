//! Render `frontend/src/generated/nav/menu.ts` — typed menu tree consumed
//! by AppSidebar / AppTopbar / AppBreadcrumb.
//!
//! Every entry references a route by NAME (compiler-checked against
//! `RouteName`).  `null` Blueprint nav → empty `NAV` const, so consumers
//! always have something to import.

use std::collections::BTreeMap;

use crate::state::{NavConfig, Role, Section};

use super::resolve::ResolvedRoute;
use super::ts::{role_list_literal, ts_string};

pub fn render(nav: Option<&NavConfig>, resolved: &[ResolvedRoute]) -> String {
    let mut out = String::new();
    out.push_str("// Auto-generated. Do not edit by hand.\n");
    out.push_str("// Typed menu tree emitted by `blast gen all`.\n");
    out.push('\n');
    out.push_str("import type { RouteName } from '@/generated/router/route-names';\n");
    out.push('\n');
    out.push_str("export type Role = 'user' | 'admin';\n");
    out.push('\n');
    out.push_str("export interface MenuEntry {\n");
    out.push_str("  readonly route: RouteName;\n");
    out.push_str("  readonly label: string | null;\n");
    out.push_str("  readonly icon: string | null;\n");
    out.push_str("  readonly roles: readonly Role[] | null;\n");
    out.push_str("}\n");
    out.push('\n');
    out.push_str("export interface MenuSection {\n");
    out.push_str("  readonly key: string;\n");
    out.push_str("  readonly label: string;\n");
    out.push_str("  readonly icon: string;\n");
    out.push_str("  readonly roles: readonly Role[] | null;\n");
    out.push_str("  readonly entries: readonly MenuEntry[];\n");
    out.push_str("}\n");
    out.push('\n');

    let by_name: BTreeMap<&str, &ResolvedRoute> =
        resolved.iter().map(|r| (r.name.as_str(), r)).collect();

    out.push_str("export const NAV: readonly MenuSection[] = [\n");
    match nav {
        Some(cfg) => {
            for section in &cfg.sections {
                push_section(&mut out, section, &by_name);
            }
        }
        None => {}
    }
    out.push_str("];\n");
    out
}

fn push_section(
    out: &mut String,
    section: &Section,
    by_name: &BTreeMap<&str, &ResolvedRoute>,
) {
    out.push_str("  {\n");
    out.push_str(&format!("    key: {},\n", ts_string(&section.key)));
    out.push_str(&format!("    label: {},\n", ts_string(&section.label)));
    out.push_str(&format!("    icon: {},\n", ts_string(&section.icon)));
    out.push_str(&format!(
        "    roles: {},\n",
        section_roles_literal(&section.roles)
    ));
    out.push_str("    entries: [\n");
    for entry in &section.entries {
        let route = by_name.get(entry.route.as_str());
        let (label, icon) = match route {
            Some(r) => (r.label.clone(), r.icon.clone()),
            None => (None, None),
        };
        out.push_str("      {\n");
        out.push_str(&format!(
            "        route: {} satisfies RouteName,\n",
            ts_string(&entry.route)
        ));
        out.push_str(&format!(
            "        label: {},\n",
            optional_string_literal(&label)
        ));
        out.push_str(&format!(
            "        icon: {},\n",
            optional_string_literal(&icon)
        ));
        out.push_str(&format!(
            "        roles: {},\n",
            entry_roles_literal(&entry.roles)
        ));
        out.push_str("      },\n");
    }
    out.push_str("    ] as const,\n");
    out.push_str("  },\n");
}

fn section_roles_literal(roles: &Option<Vec<Role>>) -> String {
    optional_roles_literal(roles)
}

fn entry_roles_literal(roles: &Option<Vec<Role>>) -> String {
    optional_roles_literal(roles)
}

fn optional_roles_literal(roles: &Option<Vec<Role>>) -> String {
    match roles {
        Some(list) if !list.is_empty() => role_list_literal(list),
        Some(_empty) => "null".to_string(), // allow: empty role list = no narrowing
        None => "null".to_string(),
    }
}

fn optional_string_literal(opt: &Option<String>) -> String {
    match opt {
        Some(s) => ts_string(s),
        None => "null".to_string(),
    }
}
