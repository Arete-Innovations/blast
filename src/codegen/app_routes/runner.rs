use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader, structs::naming::type_stem_for_resource},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{GenLevel, ResourceState, Verb},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "leptos app routes generation";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let all_resources = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };

    let mut resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Pages).collect();
    resources.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));

    let routes_dir = routes_generated_dir(project_root);
    fs::create_dir_all(&routes_dir)?;

    let mut report = EmitReport::default();
    let app_marker = header::marker_for_app(project_root)?;

    let entries = collect_route_entries(&resources);

    let table_body = render_routes_file(&entries);
    let table_path = routes_dir.join("table.rs");
    write_file(&table_path, &format!("{app_marker}{table_body}"), &mut report)?;

    let mod_body = format!("{app_marker}pub mod table;\n\npub use table::GeneratedRoutes;\n");
    let mod_path = routes_dir.join("mod.rs");
    write_file(&mod_path, &mod_body, &mut report)?;

    sink.info(format!("{STEP_LABEL}: {} routes for {} resources", entries.len(), resources.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn routes_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("routes").join("generated")
}

#[derive(Debug, Clone)]
struct RouteEntry {
    table: String,
    page_module: &'static str,
    component: String,
    path_lit: String,
}

fn collect_route_entries(resources: &[ResourceState]) -> Vec<RouteEntry> {
    let mut entries: Vec<RouteEntry> = Vec::new();
    for r in resources {
        let table = r.name.as_str();
        let stem = type_stem_for_resource(r);
        for (verb, page_module, component_suffix, path_lit) in [
            (Verb::List, "list", "ListPage", format!("/{table}")),
            (Verb::Get, "detail", "DetailPage", format!("/{table}/:id")),
            (Verb::Create, "create", "CreatePage", format!("/{table}/new")),
            (Verb::Update, "edit", "EditPage", format!("/{table}/:id/edit")),
        ] {
            let state = match r.verbs.get(&verb) {
                Some(s) => s,
                None => continue,
            };
            if !state.emit_html_page {
                continue;
            }
            entries.push(RouteEntry {
                table: table.to_string(),
                page_module,
                component: format!("{stem}{component_suffix}"),
                path_lit,
            });
        }
    }
    entries
}

fn render_routes_file(entries: &[RouteEntry]) -> String {
    let mut out = String::new();
    out.push_str("use ::leptos::prelude::*;\n");

    if entries.is_empty() {
        out.push_str("\n");
        out.push_str("#[component(transparent)]\n");
        out.push_str("pub fn GeneratedRoutes() -> impl ::leptos_router::MatchNestedRoutes + ::core::clone::Clone + ::core::marker::Send + 'static {\n");
        out.push_str("    ()\n");
        out.push_str("}\n");
        return out;
    }

    out.push_str("use ::leptos_router::components::Route;\n");
    out.push_str("use ::leptos_router::path;\n\n");

    let mut imports: Vec<String> = entries
        .iter()
        .map(|e| format!("use crate::transport::leptos::pages::generated::{}::{}::{};\n", e.table, e.page_module, e.component))
        .collect();
    imports.sort();
    imports.dedup();
    for imp in &imports {
        out.push_str(imp);
    }
    out.push('\n');

    out.push_str("#[component(transparent)]\n");
    out.push_str("pub fn GeneratedRoutes() -> impl ::leptos_router::MatchNestedRoutes + ::core::clone::Clone + ::core::marker::Send + 'static {\n");
    out.push_str("    view! {\n");
    for e in entries {
        out.push_str(&format!("        <Route path=path!(\"{}\") view={}/>\n", e.path_lit, e.component));
    }
    out.push_str("    }\n");
    out.push_str("}\n");
    out
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
        .ok_or_else(|| BlastError::Invalid(format!("app_routes target has no parent: {}", path.display())))?;
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
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;

    use super::*;
    use crate::io::null::{NullProgress, NullSink};
    use crate::state::names::{FieldName, ResourceName};
    use crate::state::resource::{AuthMode, FieldState, FieldVariant, ListOptions, RESOURCE_SCHEMA_VERSION, VerbState};
    use crate::state::{ResourceState, SqlType};

    fn make_resource(name: &str, verbs: &[(Verb, bool)]) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id".to_string()),
            FieldState {
                sql_type: SqlType::new("BigInt".to_string()),
                variants: BTreeSet::from([FieldVariant::Db, FieldVariant::Public]),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        let mut verb_map: IndexMap<Verb, VerbState> = IndexMap::new();
        for (verb, emit_html) in verbs {
            verb_map.insert(
                *verb,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: matches!(verb, Verb::List).then(|| ListOptions {
                        paginated: true,
                        filterable_columns: BTreeMap::new(),
                        sortable_columns: BTreeSet::new(),
                        default_sort: None,
                        max_page_size: None,
                    }),
                    emit_rest_api: true,
                    emit_html_page: *emit_html,
                },
            );
        }
        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new(name.to_string()),
            fields,
            verbs: verb_map,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: GenLevel::Pages,
        }
    }

    #[test]
    fn empty_resources_emit_unit_tuple() {
        let entries: Vec<RouteEntry> = collect_route_entries(&[]);
        let body = render_routes_file(&entries);
        assert!(body.contains("    ()"));
        assert!(body.contains("#[component(transparent)]"));
        assert!(body.contains("pub fn GeneratedRoutes()"));
        assert!(!body.contains("view!"));
    }

    #[test]
    fn emit_html_page_false_skips_verb() {
        let r = make_resource("posts", &[(Verb::List, false), (Verb::Get, true)]);
        let entries = collect_route_entries(&[r]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path_lit, "/posts/:id");
    }

    #[test]
    fn all_four_verbs_emit_when_flagged() {
        let r = make_resource("posts", &[(Verb::List, true), (Verb::Get, true), (Verb::Create, true), (Verb::Update, true)]);
        let entries = collect_route_entries(&[r]);
        assert_eq!(entries.len(), 4);
        let body = render_routes_file(&entries);
        assert!(body.contains("#[component(transparent)]"));
        assert!(body.contains("pub fn GeneratedRoutes()"));
        assert!(body.contains("view! {"));
        assert!(body.contains("path!(\"/posts\")"));
        assert!(body.contains("path!(\"/posts/:id\")"));
        assert!(body.contains("path!(\"/posts/new\")"));
        assert!(body.contains("path!(\"/posts/:id/edit\")"));
        assert!(body.contains("PostListPage"));
        assert!(body.contains("PostDetailPage"));
        assert!(body.contains("PostCreatePage"));
        assert!(body.contains("PostEditPage"));
    }

    #[test]
    fn delete_verb_does_not_emit_route() {
        let r = make_resource("posts", &[(Verb::Delete, true), (Verb::List, true)]);
        let entries = collect_route_entries(&[r]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].page_module, "list");
    }

    #[test]
    fn alphabetical_resource_ordering() {
        let users = make_resource("users", &[(Verb::List, true)]);
        let posts = make_resource("posts", &[(Verb::List, true)]);
        let resources = [users, posts];
        let mut sorted: Vec<ResourceState> = resources.into_iter().collect();
        sorted.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
        let entries = collect_route_entries(&sorted);
        assert_eq!(entries[0].table, "posts");
        assert_eq!(entries[1].table, "users");
    }

    #[test]
    fn idempotent_run_skips_byte_equal_files() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let project = tmp.path();
        let state_dir = project.join("storage").join("blast").join("state");
        fs::create_dir_all(state_dir.join("resources")).expect("state dir");
        let app_state = crate::state::AppState::new();
        crate::state::io::save_app(&state_dir, &app_state).expect("save app");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report1 = run(project, &mut sink, &mut progress).expect("first run");
        assert!(!report1.written.is_empty());
        let report2 = run(project, &mut sink, &mut progress).expect("second run");
        assert!(report2.written.is_empty(), "expected zero writes on second run, got {:?}", report2.written);
        assert_eq!(report2.skipped.len(), report1.written.len());
    }
}
