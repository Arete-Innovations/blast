//! Orchestration + file I/O for `blast gen models` (v2 emitter).
//!
//! For every resource state file under `storage/blast/state/resources/`:
//!
//!   1. emit `<project_root>/src/models/generated/<table>.rs` carrying the module-level fns + auto-conn `impl <Type>` wrappers + the fluent `<Type>Query` builder + IntoFuture impls (see `emitter.rs`)
//!   2. maintain a barrel `mod.rs` next to the per-resource files
//!   3. emit `CREATE INDEX` migrations for every `(filterable, sortable)` column pair declared (see `indices.rs`)
//!
//! Every emitted file carries a state-hash marker so the user app's
//! `build.rs` will refuse to compile if the on-disk state changed since the
//! last `blast gen all` run.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{
        header, ir_loader,
        models::{
            eager::Relation,
            emitter,
            indices::{self, SystemClock},
            soft_delete::SoftDeleteConfig,
        },
    },
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{GenLevel, ResourceState},
};

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "models: emit per-resource model layer";

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

    let resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Model).collect();

    let mut report = EmitReport::default();

    if resources.is_empty() {
        sink.info(format!("{STEP_LABEL}: no resources declared; nothing to emit"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let out_dir = generated_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let total = resources.len() as u64;
    for (idx, resource) in resources.iter().enumerate() {
        emit_resource(project_root, resource, &out_dir, &mut report)?;
        sink.info(format!("models: emitted {}", resource_target(&out_dir, resource).display()));
        progress.tick(idx as u64 + 1, total);
    }

    let barrel_target = out_dir.join("mod.rs");
    let barrel_marker = header::marker_for_app(project_root)?;
    let barrel_body = format!("{}{}", barrel_marker, render_barrel(&resources));
    write_file(&barrel_target, &barrel_body, &mut report)?;
    sink.info(format!("models: emitted {}", barrel_target.display()));

    // Index migrations are best-effort: failures here surface as warnings
    // but do not abort the model codegen step (the user can re-run after
    // resolving any migrations directory permission issues).
    match indices::run(project_root, &resources, &SystemClock) {
        Ok(idx_report) => {
            for path in &idx_report.written {
                report.written.push(path.clone());
                sink.info(format!("models: index migration {}", path.display()));
            }
            for path in &idx_report.skipped {
                report.skipped.push(path.clone());
            }
        }
        Err(err) => {
            sink.warn(format!("models: index migration emission failed: {} (continuing)", err));
        }
    }

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn emit_resource(project_root: &Path, resource: &ResourceState, out_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let target = resource_target(out_dir, resource);
    let marker = header::marker_for_resource(project_root, resource.name.as_str())?;
    let relations: Vec<Relation> = relations_for(resource);
    let soft_delete = soft_delete_for(resource);
    let body = format!("{}{}", marker, emitter::render_resource_body(resource, &relations, soft_delete.as_ref()),);
    write_file(&target, &body, report)
}

fn resource_target(out_dir: &Path, resource: &ResourceState) -> PathBuf {
    out_dir.join(format!("{}.rs", resource.name.as_str()))
}

fn generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("models").join("generated")
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("models target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(target, body)?;
    report.written.push(target.to_path_buf());
    Ok(())
}

fn render_barrel(resources: &[ResourceState]) -> String {
    let mut names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    names.sort();
    let mut out = String::new();
    for name in &names {
        out.push_str(&format!("pub mod {name};\n"));
    }
    out
}

/// Forward-compat shim — once `state-extensions` lands this reads
/// `resource.relations` directly.
fn relations_for(_resource: &ResourceState) -> Vec<Relation> {
    Vec::new()
}

/// Forward-compat shim — once `state-extensions` lands this reads
/// `resource.soft_delete` directly.
fn soft_delete_for(_resource: &ResourceState) -> Option<SoftDeleteConfig> {
    None
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
            names::{ResourceName, SqlType},
            AuthMode, FieldName, FieldState, FieldVariant, FilterKind, ListOptions, Verb, VerbState,
        },
    };

    fn variants(items: &[FieldVariant]) -> BTreeSet<FieldVariant> {
        items.iter().copied().collect()
    }

    fn write_full_resource(project_root: &Path, table: &str) -> BlastResult<()> {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: variants(&[FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin]),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("active"),
            FieldState {
                sql_type: SqlType::new("Bool"),
                variants: variants(&[FieldVariant::Db, FieldVariant::Public]),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("email"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: variants(&[FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public]),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
        filterable.insert(FieldName::new("email"), FilterKind::Eq);
        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: filterable,
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: Some(100),
                }),
                emit_rest_api: true,
                emit_html_page: true,
            },
        );

        let mut resource = ResourceState::new(ResourceName::new(table));
        resource.fields = fields;
        resource.verbs = verbs;

        let state_dir = project_root.join("storage").join("blast").join("state");
        crate::state::save_resource(&state_dir, &resource)?;

        let app = crate::state::AppState::default();
        crate::state::io::save_app(&state_dir, &app)?;
        Ok(())
    }

    #[test]
    fn emits_per_resource_file_with_module_fns_and_impl_block() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_full_resource(root, "users").expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("models codegen ok");

        let target = root.join("src/models/generated/users.rs");
        assert!(target.exists(), "per-resource file missing");
        assert!(report.written.iter().any(|p| p == &target));

        let body = fs::read_to_string(&target).expect("read");
        assert!(!body.starts_with("// AUTO-GENERATED"), "no inline marker — use codegen.lock.ron sidecar");
        for needle in ["pub async fn list(", "pub async fn get(", "pub async fn create(", "pub async fn update(", "pub async fn delete(", "impl User {"] {
            assert!(body.contains(needle), "missing {needle}\n{body}");
        }
    }

    #[test]
    fn emits_query_builder_with_into_future() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_full_resource(root, "users").expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let body = fs::read_to_string(root.join("src/models/generated/users.rs")).unwrap();
        assert!(body.contains("pub struct UserQuery"));
        assert!(body.contains("impl ::std::future::IntoFuture for UserQuery"));
        assert!(body.contains("impl ::std::future::IntoFuture for UserQueryPaginated"));
    }

    #[test]
    fn auto_conn_wrappers_use_pool() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_full_resource(root, "users").expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let body = fs::read_to_string(root.join("src/models/generated/users.rs")).unwrap();
        assert!(body.contains("crate::database::acquire_conn()"), "auto-conn wrappers must call crate::database::acquire_conn()");
    }

    #[test]
    fn auto_derived_scope_emitted_for_each_filter_kind() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_full_resource(root, "users").expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let body = fs::read_to_string(root.join("src/models/generated/users.rs")).unwrap();
        assert!(body.contains("pub fn active(mut self)"));
        assert!(body.contains("pub fn where_email_contains"));
    }

    #[test]
    fn empty_state_emits_nothing() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        fs::create_dir_all(root.join("storage/blast/state/resources")).expect("mkdir");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("ok");
        assert!(report.written.is_empty());
        assert!(!root.join("src/models/generated").exists());
    }

    #[test]
    fn barrel_lists_resources_lexically() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_full_resource(root, "zebras").expect("z");
        write_full_resource(root, "apples").expect("a");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let barrel = fs::read_to_string(root.join("src/models/generated/mod.rs")).unwrap();
        let apples_idx = barrel.find("pub mod apples;").unwrap();
        let zebras_idx = barrel.find("pub mod zebras;").unwrap();
        assert!(apples_idx < zebras_idx);
        assert!(!barrel.starts_with("// AUTO-GENERATED"), "no inline marker");
    }

    #[test]
    fn marker_references_resource_state_path() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_full_resource(root, "users").expect("seed");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("ok");

        let body = fs::read_to_string(root.join("src/models/generated/users.rs")).unwrap();
        assert!(!body.contains("AUTO-GENERATED"), "no inline marker");
    }
}
