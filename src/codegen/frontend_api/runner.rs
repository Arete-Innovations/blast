//! Pipeline entry point — emits per-resource TypeScript API client files
//! under `frontend/src/generated/api/`.
//!
//! When resource_count == 0, writes a `.gitkeep` so the directory exists.

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::frontend_api::render::build_resource_api;
use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "frontend api generation";

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let resources = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };

    let out_dir = api_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let mut report = EmitReport::default();

    if resources.is_empty() {
        // Emit .gitkeep so the directory exists for composables that reference it.
        let gitkeep = out_dir.join(".gitkeep");
        write_file(&gitkeep, "", &mut report)?;
        sink.info(format!(
            "{STEP_LABEL}: no resources declared; emitted .gitkeep"
        ));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    for r in &resources {
        let table = r.name.as_str();
        let marker = header::marker_for_resource(project_root, table)?;
        let body = format!("{marker}{}", build_resource_api(r));
        let path = out_dir.join(format!("{table}.ts"));
        write_file(&path, &body, &mut report)?;
        sink.info(format!("emitted {}", path.display()));
    }

    sink.info(format!(
        "{STEP_LABEL}: {} written, {} skipped",
        report.written.len(),
        report.skipped.len()
    ));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn api_dir(project_root: &Path) -> PathBuf {
    project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("api")
}

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| {
        BlastError::Invalid(format!(
            "frontend api target has no parent: {}",
            target.display()
        ))
    })?;
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
    use crate::state::resource::{
        AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState,
        RESOURCE_SCHEMA_VERSION,
    };
    use crate::state::{save_app, save_resource, AppState, SqlType};
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};
    use tempfile::TempDir;

    fn make_users_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let all_v: BTreeSet<FieldVariant> = [
            FieldVariant::Db,
            FieldVariant::Insertable,
            FieldVariant::Patch,
            FieldVariant::Public,
        ]
        .into_iter()
        .collect();
        let id_v: BTreeSet<FieldVariant> =
            [FieldVariant::Db, FieldVariant::Public].into_iter().collect();

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: id_v,
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
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("users"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
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
    fn emits_gitkeep_when_no_resources() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_project(root, &[]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run frontend api");

        let gitkeep = root.join("frontend/src/generated/api/.gitkeep");
        assert!(gitkeep.exists(), ".gitkeep must exist when no resources");
    }

    #[test]
    fn emits_per_resource_file() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_resource();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run frontend api");

        let api_file = root.join("frontend/src/generated/api/users.ts");
        assert!(api_file.exists(), "users.ts must exist");
        let body = fs::read_to_string(&api_file).expect("read users.ts");
        assert!(body.starts_with("// AUTO-GENERATED from "), "marker required");
        assert!(body.contains("export async function listUsers"), "listUsers required");
        assert!(body.contains("export async function getUser"), "getUser required");
        assert!(report.written.iter().any(|p| p == &api_file));
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

        assert!(
            second.written.is_empty(),
            "second run wrote {:?}",
            second.written
        );
        assert!(!second.skipped.is_empty(), "second run must skip files");
    }
}
