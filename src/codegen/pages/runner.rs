use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::codegen::pages::render::pages_for_resource;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state::{GenLevel, ResourceState};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "frontend pages emission";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    let all = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };
    let resources: Vec<ResourceState> = all.into_iter().filter(|r| r.gen_level >= GenLevel::Pages).collect();
    emit_for(project_root, &resources, sink, progress)
}

pub fn run_for_resource(project_root: &Path, resource_name: &str, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    let all = ir_loader::load_resource_states(project_root)?;
    let filtered: Vec<ResourceState> = all.into_iter().filter(|r| r.name.as_str() == resource_name).collect();
    if filtered.is_empty() {
        sink.warn(format!("no resource named '{resource_name}' found"));
        return Ok(EmitReport::default());
    }
    let candidate = match filtered.first() {
        Some(r) => r,
        None => return Ok(EmitReport::default()),
    };
    if candidate.gen_level < GenLevel::Pages {
        sink.warn(format!(
            "resource '{}' has gen_level {:?}, which is below Pages; skipping",
            resource_name, candidate.gen_level,
        ));
        return Ok(EmitReport::default());
    }
    emit_for(project_root, &filtered, sink, progress)
}

fn emit_for(project_root: &Path, resources: &[ResourceState], sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let pages_root = pages_root_dir(project_root);
    fs::create_dir_all(&pages_root)?;

    let mut report = EmitReport::default();
    for r in resources {
        let dir = pages_root.join(r.name.as_str());
        fs::create_dir_all(&dir)?;
        let marker = header::marker_for_resource(project_root, r.name.as_str())?;
        for (filename, body) in pages_for_resource(r) {
            let path = dir.join(&filename);
            let full = format!("{marker}{body}");
            write_file(&path, &full, &mut report)?;
            sink.info(format!("emitted {}", path.display()));
        }
    }

    sink.info(format!("{STEP_LABEL}: {} written, {} skipped", report.written.len(), report.skipped.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn pages_root_dir(project_root: &Path) -> PathBuf {
    project_root.join("frontend").join("src").join("pages").join("generated")
}

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("frontend pages target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;

    let existing = read_existing(target)?;
    match existing {
        Some(prev) if prev == body => {
            report.skipped.push(target.to_path_buf());
            return Ok(());
        }
        Some(_different) => {
            fs::write(target, body)?;
        }
        None => {
            fs::write(target, body)?;
        }
    }
    report.written.push(target.to_path_buf());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::{NullProgress, NullSink};
    use crate::state::names::{FieldName, ResourceName};
    use crate::state::resource::{AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION};
    use crate::state::{save_app, save_resource, AppState, SqlType};
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};
    use tempfile::TempDir;

    fn make_users_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let public_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public].into_iter().collect();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: public_v.clone(),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("email"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: BTreeMap::new(),
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: None,
                }),
            },
        );
        for v in [Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
            verbs.insert(
                v,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: None,
                },
            );
        }

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("users"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: crate::state::GenLevel::Pages,
        }
    }

    fn seed_project(root: &Path, resources: &[ResourceState]) {
        let state_dir = root.join("storage").join("blast").join("state");
        save_app(&state_dir, &AppState::new()).expect("save app");
        for r in resources {
            save_resource(&state_dir, r).expect("save resource");
        }
    }

    #[test]
    fn emits_all_four_pages_for_full_crud_resource() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_resource();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run pages");

        for filename in ["ListPage.vue", "DetailPage.vue", "CreatePage.vue", "EditPage.vue"] {
            let path = root.join(format!("frontend/src/pages/generated/users/{filename}"));
            assert!(path.exists(), "{filename} must exist");
            assert!(report.written.iter().any(|p| p == &path));
            let body = fs::read_to_string(&path).expect("read page");
            assert!(body.starts_with("// AUTO-GENERATED from "), "marker required in {filename}");
        }
    }

    #[test]
    fn idempotent_second_run_skips_unchanged() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_resource();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _first = run(root, &mut sink, &mut progress).expect("first run");
        let second = run(root, &mut sink, &mut progress).expect("second run");

        assert!(second.written.is_empty(), "second run wrote {:?}", second.written);
        assert!(!second.skipped.is_empty(), "second run must skip files");
    }

    #[test]
    fn run_for_resource_filters_by_name() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_resource();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run_for_resource(root, "users", &mut sink, &mut progress).expect("run for resource");
        assert_eq!(report.written.len(), 4, "users full-crud expects 4 pages");

        let report_missing = run_for_resource(root, "ghosts", &mut sink, &mut progress).expect("run for missing resource");
        assert!(report_missing.written.is_empty());
    }
}
