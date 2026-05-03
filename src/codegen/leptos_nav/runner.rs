use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader, leptos_nav::render},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{
        self,
        app::{AppPolicySection, AppState, NavConfig, Page},
        GenLevel, ResourceState, Verb,
    },
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "leptos nav generation";

const HANDWRITTEN_ROUTES: &[(&str, &str)] = &[
    ("welcome", "/"),
    ("login", "/login"),
    ("logout", "/logout"),
    ("register", "/register"),
    ("dashboard", "/dashboard"),
    ("profile", "/profile"),
];

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let state_dir = project_root.join("storage").join("blast").join("state");
    let app_state: AppState = match state::load_app(&state_dir) {
        Ok(value) => value,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };

    let resources = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };

    let nav: NavConfig = match extract_nav(&app_state) {
        Some(value) => value,
        None => NavConfig { sections: Vec::new() }, // allow: empty nav is the documented zero-state
    };
    let pages: Vec<Page> = match extract_pages(&app_state) {
        Some(value) => value,
        None => Vec::new(), // allow: zero pages is a valid scaffold state pre-`blast gen`
    };

    let route_table = build_route_table(&pages, &resources);

    validate_nav(&nav, &route_table)?;

    let nav_dir = nav_generated_dir(project_root);
    fs::create_dir_all(&nav_dir)?;

    let app_marker = header::marker_for_app(project_root)?;
    let body = render::render_app_nav(&nav, &route_table);
    let mod_body = format!("{app_marker}{body}");

    let mut report = EmitReport::default();
    let target = nav_dir.join("app_nav.rs");
    write_file(&target, &mod_body, &mut report)?;

    let barrel_target = nav_dir.join("mod.rs");
    let barrel_body = format!("{app_marker}pub mod app_nav;\n\npub use app_nav::AppNav;\n");
    write_file(&barrel_target, &barrel_body, &mut report)?;

    ensure_components_generated_includes_nav(project_root, &mut report)?;

    sink.info(format!("{STEP_LABEL}: emitted {} nav section(s) covering {} entries", nav.sections.len(), nav.sections.iter().map(|s| s.entries.len()).sum::<usize>()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn nav_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("views").join("components").join("generated").join("nav")
}

fn extract_nav(app: &AppState) -> Option<NavConfig> {
    for section in app.sections.values() {
        if let AppPolicySection::Nav(nav) = section {
            return Some(nav.clone());
        }
    }
    None
}

fn extract_pages(app: &AppState) -> Option<Vec<Page>> {
    for section in app.sections.values() {
        if let AppPolicySection::Pages(pages) = section {
            return Some(pages.clone());
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct ResolvedRoute {
    pub path: String,
}

pub fn build_route_table(pages: &[Page], resources: &[ResourceState]) -> BTreeMap<String, ResolvedRoute> {
    let mut table: BTreeMap<String, ResolvedRoute> = BTreeMap::new();

    for (name, path) in HANDWRITTEN_ROUTES {
        table.insert((*name).to_string(), ResolvedRoute { path: (*path).to_string() });
    }

    for page in pages {
        table.insert(page.route.clone(), ResolvedRoute { path: page.path.clone() });
    }

    for r in resources {
        if r.gen_level < GenLevel::Pages {
            continue;
        }
        let table_name = r.name.as_str();
        for (verb, suffix, path_template) in [
            (Verb::List, "list", format!("/{table_name}")),
            (Verb::Create, "create", format!("/{table_name}/new")),
            (Verb::Update, "edit", format!("/{table_name}/:id/edit")),
            (Verb::Get, "detail", format!("/{table_name}/:id")),
        ] {
            let state = match r.verbs.get(&verb) {
                Some(s) => s,
                None => continue,
            };
            if !state.emit_html_page {
                continue;
            }
            let route_name = format!("{table_name}.{suffix}");
            table.insert(route_name, ResolvedRoute { path: path_template });
        }
    }

    table
}

fn validate_nav(nav: &NavConfig, route_table: &BTreeMap<String, ResolvedRoute>) -> BlastResult<()> {
    for section in &nav.sections {
        for entry in &section.entries {
            if !route_table.contains_key(&entry.route) {
                let known: Vec<&str> = route_table.keys().map(String::as_str).collect();
                return Err(BlastError::Invalid(format!(
                    "nav entry references unknown route '{}' in section '{}'; known routes: {}",
                    entry.route,
                    section.key,
                    known.join(", "),
                )));
            }
        }
    }
    Ok(())
}

fn ensure_components_generated_includes_nav(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src").join("views").join("components").join("generated").join("mod.rs");
    let existing = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_io) => return Ok(()),
    };
    if existing.contains("pub mod nav;") {
        return Ok(());
    }
    let updated = if existing.ends_with('\n') {
        format!("{existing}pub mod nav;\n")
    } else {
        format!("{existing}\npub mod nav;\n")
    };
    fs::write(&path, &updated)?;
    report.written.push(path);
    Ok(())
}

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(path: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| BlastError::Invalid(format!("leptos_nav target has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;

    let existing = read_existing(path)?;
    match existing {
        Some(prev) if prev == body => {
            report.skipped.push(path.to_path_buf());
            return Ok(());
        }
        Some(_different) => {
            fs::write(path, body)?;
        }
        None => {
            fs::write(path, body)?;
        }
    }
    report.written.push(path.to_path_buf());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use indexmap::IndexMap;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        io::null::{NullProgress, NullSink},
        state::{
            app::{Entry as NavEntry, NavConfig, Page, PageLayout, Role as StateRole, Section as NavSection},
            names::{FieldName, ResourceName},
            resource::{AuthMode, FieldState, FieldVariant, ListOptions, RESOURCE_SCHEMA_VERSION, VerbState},
            save_app, save_resource, AppPolicySection, AppState, GenLevel, ResourceState, SqlType, Verb,
        },
    };

    fn empty_resource(name: &str, verbs: &[(Verb, bool)]) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("BigInt"),
                variants: [FieldVariant::Db, FieldVariant::Public].into_iter().collect(),
                nullable: false,
                primary_key: true,
                validators: Default::default(),
            },
        );
        let mut verb_map: IndexMap<Verb, VerbState> = IndexMap::new();
        for (verb, emit) in verbs {
            verb_map.insert(
                *verb,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: matches!(verb, Verb::List).then(|| ListOptions {
                        paginated: true,
                        filterable_columns: BTreeMap::new(),
                        sortable_columns: Default::default(),
                        default_sort: None,
                        max_page_size: None,
                    }),
                    emit_rest_api: true,
                    emit_html_page: *emit,
                },
            );
        }
        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new(name),
            fields,
            verbs: verb_map,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: GenLevel::Pages,
        }
    }

    fn seed_project(root: &Path, app: AppState, resources: &[ResourceState]) {
        let state_dir = root.join("storage").join("blast").join("state");
        save_app(&state_dir, &app).expect("save app");
        for r in resources {
            save_resource(&state_dir, r).expect("save resource");
        }
    }

    fn nav_app(nav: NavConfig, pages: Vec<Page>) -> AppState {
        let mut app = AppState::new();
        app.sections.insert("nav".to_string(), AppPolicySection::Nav(nav));
        app.sections.insert("pages".to_string(), AppPolicySection::Pages(pages));
        app
    }

    fn dashboard_only_nav() -> NavConfig {
        NavConfig {
            sections: vec![NavSection {
                key: "main".to_string(),
                label: "Main".to_string(),
                icon: "home".to_string(),
                roles: None,
                entries: vec![NavEntry {
                    route: "dashboard".to_string(),
                    label: Some("Dashboard".to_string()),
                    icon: None,
                    roles: None,
                }],
            }],
        }
    }

    #[test]
    fn empty_nav_emits_skeleton() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_project(root, AppState::new(), &[]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run leptos_nav");

        let body = fs::read_to_string(root.join("src/views/components/generated/nav/app_nav.rs")).expect("read nav");
        assert!(body.contains("pub fn AppNav"), "body must contain AppNav: {body}");
        assert!(!report.written.is_empty(), "expect at least one write");
    }

    #[test]
    fn handwritten_route_resolves_for_dashboard() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_project(root, nav_app(dashboard_only_nav(), Vec::new()), &[]);

        run(root, &mut NullSink, &mut NullProgress).expect("run leptos_nav");

        let body = fs::read_to_string(root.join("src/views/components/generated/nav/app_nav.rs")).expect("read nav");
        assert!(body.contains("\"/dashboard\""), "must hardcode /dashboard path: {body}");
        assert!(body.contains("\"Dashboard\""), "must include label: {body}");
    }

    #[test]
    fn unknown_route_errors() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let nav = NavConfig {
            sections: vec![NavSection {
                key: "main".to_string(),
                label: "Main".to_string(),
                icon: "home".to_string(),
                roles: None,
                entries: vec![NavEntry {
                    route: "does.not.exist".to_string(),
                    label: None,
                    icon: None,
                    roles: None,
                }],
            }],
        };
        seed_project(root, nav_app(nav, Vec::new()), &[]);

        let err = run(root, &mut NullSink, &mut NullProgress).expect_err("expected unknown-route failure");
        match err {
            BlastError::Invalid(msg) => assert!(msg.contains("does.not.exist"), "msg should mention bad route: {msg}"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn resource_route_resolves_to_list_path() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let nav = NavConfig {
            sections: vec![NavSection {
                key: "main".to_string(),
                label: "Main".to_string(),
                icon: "home".to_string(),
                roles: None,
                entries: vec![NavEntry {
                    route: "posts.list".to_string(),
                    label: Some("Posts".to_string()),
                    icon: None,
                    roles: None,
                }],
            }],
        };
        let posts = empty_resource("posts", &[(Verb::List, true)]);
        seed_project(root, nav_app(nav, Vec::new()), &[posts]);

        run(root, &mut NullSink, &mut NullProgress).expect("run leptos_nav");

        let body = fs::read_to_string(root.join("src/views/components/generated/nav/app_nav.rs")).expect("read nav");
        assert!(body.contains("\"/posts\""), "must include /posts path literal: {body}");
        assert!(body.contains("\"Posts\""), "must include override label: {body}");
    }

    #[test]
    fn role_gating_emits_show_when() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let nav = NavConfig {
            sections: vec![NavSection {
                key: "ops".to_string(),
                label: "Ops".to_string(),
                icon: "tools".to_string(),
                roles: Some(vec![StateRole::Admin]),
                entries: vec![NavEntry {
                    route: "dashboard".to_string(),
                    label: None,
                    icon: None,
                    roles: Some(vec![StateRole::Admin]),
                }],
            }],
        };
        seed_project(root, nav_app(nav, Vec::new()), &[]);

        run(root, &mut NullSink, &mut NullProgress).expect("run leptos_nav");

        let body = fs::read_to_string(root.join("src/views/components/generated/nav/app_nav.rs")).expect("read nav");
        assert!(body.contains("Show"), "role-gated section must use Show: {body}");
        assert!(body.contains("Role::Admin"), "must reference canonical Role::Admin: {body}");
    }

    #[test]
    fn idempotent_run() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_project(root, nav_app(dashboard_only_nav(), Vec::new()), &[]);

        let _first = run(root, &mut NullSink, &mut NullProgress).expect("first");
        let second = run(root, &mut NullSink, &mut NullProgress).expect("second");
        assert!(!second.skipped.is_empty(), "second run must skip unchanged files");
    }

    #[test]
    fn pages_path_takes_precedence() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let pages = vec![Page {
            route: "custom_page".to_string(),
            path: "/custom".to_string(),
            component: "custom:CustomPage".to_string(),
            layout: PageLayout::Cards,
            label: Some("Custom".to_string()),
            icon: None,
            in_nav: true,
            roles: None,
        }];
        let nav = NavConfig {
            sections: vec![NavSection {
                key: "main".to_string(),
                label: "Main".to_string(),
                icon: "home".to_string(),
                roles: None,
                entries: vec![NavEntry {
                    route: "custom_page".to_string(),
                    label: None,
                    icon: None,
                    roles: None,
                }],
            }],
        };
        seed_project(root, nav_app(nav, pages), &[]);

        run(root, &mut NullSink, &mut NullProgress).expect("run leptos_nav");

        let body = fs::read_to_string(root.join("src/views/components/generated/nav/app_nav.rs")).expect("read nav");
        assert!(body.contains("\"/custom\""), "must use Page-declared path: {body}");
    }
}
