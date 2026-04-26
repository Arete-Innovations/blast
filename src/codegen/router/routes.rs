//! Render `frontend/src/generated/router/routes.ts` — a `RouteRecordRaw[]`
//! consumable by `vue-router`. Components are lazy-loaded via dynamic
//! `import()` so the bundler code-splits per route.

use super::resolve::ResolvedRoute;
use super::ts::{auth_roles_literal, page_layout_literal, ts_string};

pub fn render(routes: &[ResolvedRoute]) -> String {
    let mut out = String::new();
    out.push_str("// Auto-generated. Do not edit by hand.\n");
    out.push_str("// vue-router config emitted by `blast gen all`.\n");
    out.push('\n');
    out.push_str("import type { RouteRecordRaw } from 'vue-router';\n");
    out.push('\n');
    out.push_str("import type { RouteName } from './route-names';\n");
    out.push('\n');

    out.push_str("export interface RouteMeta {\n");
    out.push_str("  readonly layout: 'cards' | 'split' | 'table' | 'bleed' | 'tabbed';\n");
    out.push_str("  readonly label: string | null;\n");
    out.push_str("  readonly icon: string | null;\n");
    out.push_str("  readonly roles: readonly ('user' | 'admin')[] | null;\n");
    out.push_str("}\n");
    out.push('\n');

    out.push_str("export const routes: readonly RouteRecordRaw[] = [\n");
    for r in routes {
        push_route_object(&mut out, r);
    }
    out.push_str("];\n");
    out
}

fn push_route_object(out: &mut String, r: &ResolvedRoute) {
    let name_lit = ts_string(&r.name);
    let path_lit = ts_string(&r.path);
    let component_lit = ts_string(&r.component_import);
    let layout_lit = page_layout_literal(&r.layout);
    let label_lit = match &r.label {
        Some(s) => ts_string(s),
        None => "null".to_string(),
    };
    let icon_lit = match &r.icon {
        Some(s) => ts_string(s),
        None => "null".to_string(),
    };
    let roles_lit = auth_roles_literal(&r.auth);

    out.push_str("  {\n");
    out.push_str(&format!(
        "    name: {name} satisfies RouteName,\n",
        name = name_lit
    ));
    out.push_str(&format!("    path: {path},\n", path = path_lit));
    out.push_str(&format!(
        "    component: () => import({component}),\n",
        component = component_lit
    ));
    out.push_str("    meta: {\n");
    out.push_str(&format!("      layout: {},\n", layout_lit));
    out.push_str(&format!("      label: {},\n", label_lit));
    out.push_str(&format!("      icon: {},\n", icon_lit));
    out.push_str(&format!("      roles: {},\n", roles_lit));
    out.push_str("    } satisfies RouteMeta,\n");
    out.push_str("  },\n");
}
