//! Emit `<AdminCrudMenu/>` into `views/components/generated/admin/`.
//!
//! Single-component pass; consumes resources at `gen_level >= Pages` and
//! renders one card per resource with links to its list/create pages.
//! Ensures the `views/components/generated/mod.rs` barrel pulls in the
//! `admin` subdir so the user-app sees `views::components::AdminCrudMenu`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader, leptos_admin::render::{collect_entries, render_admin_crud_menu}},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{CrankPolicy, GenLevel, ResourceState},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "leptos admin menu generation";

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

    let resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Pages).collect();

    let admin_dir = admin_generated_dir(project_root);
    fs::create_dir_all(&admin_dir)?;

    let app_marker = header::marker_for_app(project_root)?;
    let entries = collect_entries(&resources);
    let body = render_admin_crud_menu(&entries);

    let mut report = EmitReport::default();
    let target = admin_dir.join("admin_crud_menu.rs");
    write_file(&target, &format!("{app_marker}{body}"), &mut report)?;

    let barrel_target = admin_dir.join("mod.rs");
    let barrel_body = format!("{app_marker}pub mod admin_crud_menu;\n\npub use admin_crud_menu::AdminCrudMenu;\n");
    write_file(&barrel_target, &barrel_body, &mut report)?;

    ensure_components_generated_includes_admin(project_root, &mut report)?;

    sink.info(format!("{STEP_LABEL}: emitted menu with {} resource(s)", entries.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn admin_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("views").join("components").join("generated").join("admin")
}

fn ensure_components_generated_includes_admin(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src").join("views").join("components").join("generated").join("mod.rs");
    let existing = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_io) => return Ok(()),
    };
    if existing.contains("pub mod admin;") {
        return Ok(());
    }
    let updated = if existing.ends_with('\n') {
        format!("{existing}pub mod admin;\n")
    } else {
        format!("{existing}\npub mod admin;\n")
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
        .ok_or_else(|| BlastError::Invalid(format!("leptos_admin target has no parent: {}", path.display())))?;
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
    use tempfile::TempDir;

    use super::*;
    use crate::{
        io::null::{NullProgress, NullSink},
        state::{
            names::{FieldName, ResourceName, SqlType},
            resource::{AuthMode, FieldState, FieldVariant, ListOptions, RESOURCE_SCHEMA_VERSION, VerbState},
            save_app, save_resource, AppState, ResourceState, Verb,
        },
    };

    fn seed(root: &Path, resources: &[ResourceState]) {
        let state_dir = root.join("storage").join("blast").join("state");
        save_app(&state_dir, &AppState::new()).expect("save app");
        for r in resources {
            save_resource(&state_dir, r).expect("save resource");
        }
    }

    fn pages_resource(table: &str, verbs: &[Verb]) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: [FieldVariant::Db, FieldVariant::Public].into_iter().collect(),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            
            kind: Default::default(),
        },
        );
        let mut verb_map: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in verbs {
            verb_map.insert(
                *v,
                VerbState {
                    auth: AuthMode::AuthRequired,
                    list_options: matches!(v, Verb::List).then(|| ListOptions {
                        paginated: true,
                        filterable_columns: BTreeMap::new(),
                        sortable_columns: BTreeSet::new(),
                        default_sort: None,
                        max_page_size: None,
                    }),
                    emit_rest_api: true,
                    emit_html_page: true,
                                    crank_policy: CrankPolicy::None,
                },
            );
        }
        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new(table),
            fields,
            verbs: verb_map,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: GenLevel::Pages,
            list_layout: None,
            detail_layout: None,
            toggle_endpoint: None,
            live_topics: Vec::new(),
        }
    }

    #[test]
    fn empty_resources_emits_placeholder_component() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed(root, &[]);
        run(root, &mut NullSink, &mut NullProgress).expect("admin run");

        let body = fs::read_to_string(root.join("src/views/components/generated/admin/admin_crud_menu.rs")).expect("read");
        assert!(body.contains("pub fn AdminCrudMenu"));
        assert!(body.contains("admin-crud-menu--empty"));
    }

    #[test]
    fn populated_resources_emit_links() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed(root, &[pages_resource("postari", &[Verb::List, Verb::Get, Verb::Create, Verb::Delete])]);
        run(root, &mut NullSink, &mut NullProgress).expect("admin run");

        let body = fs::read_to_string(root.join("src/views/components/generated/admin/admin_crud_menu.rs")).expect("read");
        assert!(body.contains("href=\"/postari\""), "list link: {body}");
        assert!(body.contains("href=\"/postari/new\""), "create link: {body}");
        assert!(body.contains("\"Postari\""), "stem heading: {body}");
    }

    #[test]
    fn ensures_components_generated_barrel_pulls_admin() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed(root, &[]);
        let barrel = root.join("src/views/components/generated/mod.rs");
        fs::create_dir_all(barrel.parent().unwrap()).expect("mkdir");
        fs::write(&barrel, "pub mod forms;\npub mod nav;\n").expect("seed barrel");
        run(root, &mut NullSink, &mut NullProgress).expect("admin run");
        let updated = fs::read_to_string(&barrel).expect("read");
        assert!(updated.contains("pub mod admin;"), "barrel must pull admin:\n{updated}");
    }

    #[test]
    fn idempotent_run() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed(root, &[pages_resource("widgets", &[Verb::List])]);
        let _first = run(root, &mut NullSink, &mut NullProgress).expect("first");
        let second = run(root, &mut NullSink, &mut NullProgress).expect("second");
        assert!(!second.skipped.is_empty(), "second run must skip unchanged files");
    }
}
