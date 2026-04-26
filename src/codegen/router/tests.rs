#[cfg(test)]
use super::resolve;
use super::resolve::ResolvedRoute;
use super::runner::run;
use super::{guards, menu, route_names, routes, validate};

use crate::io::null::{NullProgress, NullSink};
use crate::state::{
    AppPolicySection, AppState, AuthMode, Entry, FieldName, FieldState, FieldVariant, NavConfig,
    Page, PageLayout, ResourceName, ResourceState, Role, Section, SqlType, Verb, VerbState,
};
use indexmap::IndexMap;
use std::collections::BTreeSet;
use std::fs;
use tempfile::TempDir;

fn build_resolved_for_test(resources: &[ResourceState], pages: &[Page]) -> Vec<ResolvedRoute> {
    resolve::resolve_all(resources, pages)
}

// ── helpers ────────────────────────────────────────────────────────────────

fn make_resource(table: &str, verbs: &[(Verb, AuthMode)]) -> ResourceState {
    let mut field_variants = BTreeSet::new();
    field_variants.insert(FieldVariant::Db);
    field_variants.insert(FieldVariant::Public);
    let id_field = FieldState {
        sql_type: SqlType::new("BIGSERIAL"),
        variants: field_variants,
        nullable: false,
        primary_key: true,
        validators: BTreeSet::new(),
    };
    let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
    fields.insert(FieldName::new("id"), id_field);

    let mut vmap: IndexMap<Verb, VerbState> = IndexMap::new();
    for (verb, auth) in verbs {
        vmap.insert(
            *verb,
            VerbState {
                auth: auth.clone(),
                list_options: None,
            },
        );
    }
    let mut res = ResourceState::new(ResourceName::new(table));
    res.fields = fields;
    res.verbs = vmap;
    res
}

fn write_app_with(dir: &TempDir, sections: Vec<(&str, AppPolicySection)>) {
    let mut state = AppState::new();
    for (k, v) in sections {
        state.sections.insert(k.to_string(), v);
    }
    let state_dir = dir.path().join("storage/blast/state");
    fs::create_dir_all(&state_dir).expect("mkdir");
    crate::state::save_app(&state_dir, &state).expect("save_app");
}

fn write_resource(dir: &TempDir, res: &ResourceState) {
    let state_dir = dir.path().join("storage/blast/state");
    crate::state::save_resource(&state_dir, res).expect("save_resource");
}

// ── resolve: CRUD route auto-emission per Primer verb ──────────────────────

#[test]
fn crud_routes_emit_one_per_enabled_verb() {
    let res = make_resource(
        "users",
        &[
            (Verb::List, AuthMode::AuthRequired),
            (Verb::Get, AuthMode::AuthRequired),
            (Verb::Create, AuthMode::AdminOnly),
            (Verb::Update, AuthMode::AuthRequired),
        ],
    );
    let resolved = build_resolved_for_test(&[res], &[]);
    let names: Vec<&str> = resolved.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"users.list"));
    assert!(names.contains(&"users.detail"));
    assert!(names.contains(&"users.create"));
    assert!(names.contains(&"users.edit"));
}

#[test]
fn delete_verb_emits_no_route() {
    let res = make_resource("orders", &[(Verb::Delete, AuthMode::AdminOnly)]);
    let resolved = build_resolved_for_test(&[res], &[]);
    assert_eq!(resolved.len(), 0);
}

#[test]
fn crud_route_paths_match_spec_table_layouts() {
    let res = make_resource(
        "orders",
        &[
            (Verb::List, AuthMode::Public),
            (Verb::Get, AuthMode::Public),
            (Verb::Create, AuthMode::AdminOnly),
            (Verb::Update, AuthMode::AuthRequired),
        ],
    );
    let resolved = build_resolved_for_test(&[res], &[]);
    let lookup = |name: &str| resolved.iter().find(|r| r.name == name).cloned().unwrap();

    let list = lookup("orders.list");
    assert_eq!(list.path, "/orders");
    assert!(matches!(list.layout, PageLayout::Table));
    assert_eq!(list.component_import, "@/pages/orders/ListPage.vue");

    let detail = lookup("orders.detail");
    assert_eq!(detail.path, "/orders/:id");
    assert!(matches!(detail.layout, PageLayout::Cards));
    assert_eq!(detail.component_import, "@/pages/orders/DetailPage.vue");

    let create = lookup("orders.create");
    assert_eq!(create.path, "/orders/new");
    assert_eq!(create.component_import, "@/pages/orders/CreatePage.vue");

    let edit = lookup("orders.edit");
    assert_eq!(edit.path, "/orders/:id/edit");
    assert_eq!(edit.component_import, "@/pages/orders/EditPage.vue");
}

// ── resolve: custom page emission from Blueprint ───────────────────────────

#[test]
fn custom_page_emission_carries_layout_and_roles() {
    let pages = vec![Page {
        route: "dashboard".to_string(),
        path: "/".to_string(),
        component: "custom/pages/DashboardPage.vue".to_string(),
        layout: PageLayout::Cards,
        label: Some("Dashboard".to_string()),
        icon: Some("dashboard".to_string()),
        in_nav: true,
        roles: Some(vec![Role::User, Role::Admin]),
    }];
    let resolved = build_resolved_for_test(&[], &pages);
    assert_eq!(resolved.len(), 1);
    let r = &resolved[0];
    assert_eq!(r.name, "dashboard");
    assert_eq!(r.path, "/");
    assert_eq!(r.component_import, "@/custom/pages/DashboardPage.vue");
    assert!(matches!(r.layout, PageLayout::Cards));
    assert_eq!(r.label.as_deref(), Some("Dashboard"));
}

// ── validate: dangling route → BlastError ──────────────────────────────────

#[test]
fn dangling_nav_entry_route_fails_codegen() {
    let nav = NavConfig {
        sections: vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "missing.route".to_string(),
                roles: None,
            }],
        }],
    };
    let resolved: Vec<ResolvedRoute> = Vec::new();
    let err = validate::validate_nav_against_routes(Some(&nav), &resolved)
        .expect_err("expected dangling-route error");
    let msg = err.to_string();
    assert!(msg.contains("missing.route"), "msg: {msg}");
    assert!(msg.contains("main"), "msg: {msg}");
}

#[test]
fn nav_entry_roles_must_be_subset_of_route_auth() {
    // Route admin-only; entry asks for user → reject.
    let pages = vec![Page {
        route: "ops".to_string(),
        path: "/ops".to_string(),
        component: "custom/pages/Ops.vue".to_string(),
        layout: PageLayout::Cards,
        label: None,
        icon: None,
        in_nav: true,
        roles: Some(vec![Role::Admin]),
    }];
    let resolved = build_resolved_for_test(&[], &pages);
    let nav = NavConfig {
        sections: vec![Section {
            key: "ops".to_string(),
            label: "Ops".to_string(),
            icon: "tools".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "ops".to_string(),
                roles: Some(vec![Role::User]),
            }],
        }],
    };
    let err = validate::validate_nav_against_routes(Some(&nav), &resolved)
        .expect_err("expected role-subset error");
    assert!(err.to_string().contains("subset"), "{}", err);
}

#[test]
fn nav_entry_role_subset_matching_passes() {
    let pages = vec![Page {
        route: "audit.list".to_string(),
        path: "/audit".to_string(),
        component: "custom/pages/AuditList.vue".to_string(),
        layout: PageLayout::Table,
        label: None,
        icon: None,
        in_nav: true,
        roles: Some(vec![Role::User, Role::Admin]),
    }];
    let resolved = build_resolved_for_test(&[], &pages);
    let nav = NavConfig {
        sections: vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: None,
            entries: vec![Entry {
                route: "audit.list".to_string(),
                roles: Some(vec![Role::Admin]),
            }],
        }],
    };
    validate::validate_nav_against_routes(Some(&nav), &resolved)
        .expect("subset of {User,Admin} should validate");
}

// ── route-names: union exhaustiveness ──────────────────────────────────────

#[test]
fn route_names_emits_union_of_all_resolved() {
    let res = make_resource("users", &[(Verb::List, AuthMode::Public)]);
    let pages = vec![Page {
        route: "dashboard".to_string(),
        path: "/".to_string(),
        component: "custom/pages/Dashboard.vue".to_string(),
        layout: PageLayout::Cards,
        label: Some("Dashboard".to_string()),
        icon: None,
        in_nav: true,
        roles: None,
    }];
    let resolved = build_resolved_for_test(&[res], &pages);
    let body = route_names::render(&resolved);
    assert!(body.contains("export type RouteName ="));
    assert!(body.contains("'users.list'"));
    assert!(body.contains("'dashboard'"));
    assert!(body.contains("export const ROUTE_NAMES"));
}

#[test]
fn route_names_empty_emits_never() {
    let body = route_names::render(&[]);
    assert!(body.contains("RouteName = never"));
    assert!(body.contains("ROUTE_NAMES = {}"));
}

// ── menu: hierarchy round-trip ─────────────────────────────────────────────

#[test]
fn menu_renders_section_and_entries_with_route_meta_inheritance() {
    let pages = vec![Page {
        route: "dashboard".to_string(),
        path: "/".to_string(),
        component: "custom/pages/Dashboard.vue".to_string(),
        layout: PageLayout::Cards,
        label: Some("Home".to_string()),
        icon: Some("home".to_string()),
        in_nav: true,
        roles: None,
    }];
    let resolved = build_resolved_for_test(&[], &pages);
    let nav = NavConfig {
        sections: vec![Section {
            key: "main".to_string(),
            label: "Main".to_string(),
            icon: "home".to_string(),
            roles: Some(vec![Role::User, Role::Admin]),
            entries: vec![Entry {
                route: "dashboard".to_string(),
                roles: None,
            }],
        }],
    };
    let body = menu::render(Some(&nav), &resolved);
    assert!(body.contains("export const NAV"));
    assert!(body.contains("'main'"));
    assert!(body.contains("'dashboard'"));
    // entry inherits label/icon from the route
    assert!(body.contains("'Home'"));
    assert!(body.contains("'home'"));
}

#[test]
fn menu_with_no_nav_emits_empty_const() {
    let body = menu::render(None, &[]);
    assert!(body.contains("export const NAV: readonly MenuSection[] = [\n];\n"));
}

// ── routes.ts: lazy-import + meta ──────────────────────────────────────────

#[test]
fn routes_emits_lazy_imports_and_meta() {
    let res = make_resource("users", &[(Verb::List, AuthMode::AdminOnly)]);
    let resolved = build_resolved_for_test(&[res], &[]);
    let body = routes::render(&resolved);
    assert!(body.contains("import type { RouteRecordRaw } from 'vue-router'"));
    assert!(body.contains("() => import('@/pages/users/ListPage.vue')"));
    assert!(body.contains("name: 'users.list' satisfies RouteName"));
    assert!(body.contains("path: '/users'"));
    assert!(body.contains("layout: 'table'"));
    assert!(body.contains("roles: ['admin'] as const"));
}

#[test]
fn routes_public_auth_emits_null_roles() {
    let res = make_resource("orders", &[(Verb::List, AuthMode::Public)]);
    let resolved = build_resolved_for_test(&[res], &[]);
    let body = routes::render(&resolved);
    assert!(body.contains("roles: null"));
}

#[test]
fn routes_required_auth_emits_empty_array_const() {
    let res = make_resource("notes", &[(Verb::List, AuthMode::AuthRequired)]);
    let resolved = build_resolved_for_test(&[res], &[]);
    let body = routes::render(&resolved);
    assert!(body.contains("roles: [] as const"));
}

// ── guards.ts: shape ───────────────────────────────────────────────────────

#[test]
fn guards_exports_install_function_with_resolve_role_param() {
    let body = guards::render();
    assert!(body.contains("export function installRouterGuards"));
    assert!(body.contains("resolveRole?: ()"));
    assert!(body.contains("opts?: InstallRouterGuardsOptions"));
    assert!(body.contains("router.beforeEach"));
    assert!(!body.contains("console.log"));
    assert!(!body.contains(": any"));
    assert!(!body.contains("@ts-ignore"));
}

// ── End-to-end: run() emits all four files with hash markers ───────────────

#[test]
fn run_emits_all_four_artifacts_with_hash_markers() {
    let dir = TempDir::new().expect("tempdir");
    let res = make_resource(
        "users",
        &[(Verb::List, AuthMode::Public), (Verb::Get, AuthMode::Public)],
    );
    write_resource(&dir, &res);
    write_app_with(
        &dir,
        vec![(
            "pages",
            AppPolicySection::Pages(vec![Page {
                route: "dashboard".to_string(),
                path: "/".to_string(),
                component: "custom/pages/Dashboard.vue".to_string(),
                layout: PageLayout::Cards,
                label: Some("Dashboard".to_string()),
                icon: None,
                in_nav: true,
                roles: None,
            }]),
        )],
    );

    let mut sink = NullSink;
    let mut progress = NullProgress;
    let report = run(dir.path(), &mut sink, &mut progress).expect("run");
    assert_eq!(report.written.len(), 4);

    let routes_body =
        fs::read_to_string(dir.path().join("frontend/src/generated/router/routes.ts")).unwrap();
    assert!(routes_body.starts_with("// AUTO-GENERATED from storage/blast/state/app.ron"));
    assert!(routes_body.contains("'users.list'"));
    assert!(routes_body.contains("'dashboard'"));

    let names_body = fs::read_to_string(
        dir.path().join("frontend/src/generated/router/route-names.ts"),
    )
    .unwrap();
    assert!(names_body.starts_with("// AUTO-GENERATED from storage/blast/state/app.ron"));
    assert!(names_body.contains("export type RouteName ="));

    let guards_body = fs::read_to_string(
        dir.path()
            .join("frontend/src/generated/router/install-router-guards.ts"),
    )
    .unwrap();
    assert!(guards_body.starts_with("// AUTO-GENERATED from storage/blast/state/app.ron"));
    assert!(guards_body.contains("installRouterGuards"));

    let menu_body =
        fs::read_to_string(dir.path().join("frontend/src/generated/nav/menu.ts")).unwrap();
    assert!(menu_body.starts_with("// AUTO-GENERATED from storage/blast/state/app.ron"));
    assert!(menu_body.contains("export const NAV"));
}

#[test]
fn run_with_dangling_nav_returns_invalid_error() {
    let dir = TempDir::new().expect("tempdir");
    let res = make_resource("users", &[(Verb::List, AuthMode::Public)]);
    write_resource(&dir, &res);
    write_app_with(
        &dir,
        vec![(
            "nav",
            AppPolicySection::Nav(NavConfig {
                sections: vec![Section {
                    key: "main".to_string(),
                    label: "Main".to_string(),
                    icon: "home".to_string(),
                    roles: None,
                    entries: vec![Entry {
                        route: "ghost.route".to_string(),
                        roles: None,
                    }],
                }],
            }),
        )],
    );

    let mut sink = NullSink;
    let mut progress = NullProgress;
    let err = run(dir.path(), &mut sink, &mut progress).expect_err("dangling route");
    let msg = err.to_string();
    assert!(msg.contains("ghost.route"), "{msg}");
}
