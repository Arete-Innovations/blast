//! Render `frontend/src/generated/router/route-names.ts` — a string-union
//! type that turns `{ name: 'users.list' }` into a compiler-checked
//! lookup. Plus a `ROUTE_NAMES` const object: `ROUTE_NAMES['users.list']`
//! returns the matching string literal (typed as `RouteName`). Object
//! form lets generated CRUD pages do `ROUTE_NAMES[<name>]` lookups
//! without casting.

use super::resolve::ResolvedRoute;
use super::ts::ts_string;

pub fn render(routes: &[ResolvedRoute]) -> String {
    let mut out = String::new();
    out.push_str("// Auto-generated. Do not edit by hand.\n");
    out.push_str("// Compiler-checked named-route lookup.\n");
    out.push('\n');

    if routes.is_empty() {
        out.push_str("export type RouteName = never;\n");
        out.push('\n');
        out.push_str("export const ROUTE_NAMES = {} as const;\n");
        return out;
    }

    out.push_str("export type RouteName =\n");
    for (idx, r) in routes.iter().enumerate() {
        let lit = ts_string(&r.name);
        let terminator = if idx + 1 == routes.len() { ";" } else { "" };
        out.push_str(&format!("  | {}{}\n", lit, terminator));
    }
    out.push('\n');

    out.push_str("export const ROUTE_NAMES = {\n");
    for r in routes {
        let lit = ts_string(&r.name);
        out.push_str(&format!("  {}: {},\n", lit, lit));
    }
    out.push_str("} as const satisfies Readonly<Record<RouteName, RouteName>>;\n");
    out
}
