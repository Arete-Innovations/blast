use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader, routines::emitter},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{GenLevel, ResourceState, Verb},
};

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "routines: emit per-resource verb stubs";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let all_resources = ir_loader::load_resource_states(project_root)?;
    let resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Route).collect();
    let mut report = EmitReport::default();

    if resources.is_empty() {
        sink.info("routines: no resources at Route+ gen_level; nothing to emit");
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let out_dir = generated_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let total = resources.len() as u64;
    for (idx, resource) in resources.iter().enumerate() {
        emit_resource(project_root, resource, &out_dir, &mut report)?;
        progress.tick(idx as u64 + 1, total);
    }

    let tables: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    let barrel_target = out_dir.join("mod.rs");
    let barrel_marker = header::marker_for_app(project_root)?;
    let barrel_body = format!("{}{}", barrel_marker, emitter::render_top_barrel(&tables));
    write_file(&barrel_target, &barrel_body, &mut report)?;

    sink.info(format!("routines: {} file(s) written across {} resource(s)", report.written.len(), resources.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn emit_resource(project_root: &Path, resource: &ResourceState, out_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let table = resource.name.as_str();
    let resource_dir = out_dir.join(table);
    fs::create_dir_all(&resource_dir)?;

    let marker = header::marker_for_resource(project_root, table)?;
    let verbs: Vec<Verb> = resource.verbs.keys().copied().collect();

    write_file(&resource_dir.join("mod.rs"), &format!("{}{}", marker, emitter::render_resource_barrel(&verbs)), report)?;

    for verb in &verbs {
        let body = emitter::render_verb_body(table, *verb);
        write_file(&resource_dir.join(format!("{}.rs", emitter::verb_module(*verb))), &format!("{}{}", marker, body), report)?;
    }

    Ok(())
}

fn generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("routines").join("generated")
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("routines target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    let mut file = fs::File::create(target)?;
    file.write_all(body.as_bytes())?;
    report.written.push(target.to_path_buf());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs as stdfs};

    use indexmap::IndexMap;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        io::null::{NullProgress, NullSink},
        state::{resource::RESOURCE_SCHEMA_VERSION, AuthMode, FieldName, FieldState, FieldVariant, ResourceName, ResourceState, SqlType, Verb, VerbState},
    };

    fn write_resource_ron(project_root: &Path, name: &str) {
        let resources_dir = project_root.join("storage/blast/state/resources");
        stdfs::create_dir_all(&resources_dir).expect("mkdir resources");
        let state_dir = project_root.join("storage/blast/state");
        let app = crate::state::AppState::default();
        crate::state::io::save_app(&state_dir, &app).expect("save app");
        let mut variants = BTreeSet::new();
        variants.insert(FieldVariant::Public);
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("BIGINT"),
                variants: variants.clone(),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in [Verb::List, Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
            verbs.insert(
                v,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: None,
                    emit_rest_api: true,
                    emit_html_page: true,
                },
            );
        }
        let resource = ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new(name),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: std::collections::BTreeMap::new(),
            gen_level: crate::state::GenLevel::Route,
        };
        let path = resources_dir.join(format!("{}.ron", name));
        let body = ron::ser::to_string_pretty(&resource, ron::ser::PrettyConfig::default()).expect("serialize resource");
        stdfs::write(&path, body).expect("write resource ron");
    }

    #[test]
    fn emits_per_resource_dir_and_top_barrel() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_resource_ron(root, "users");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("routines codegen ok");

        let users_dir = root.join("src/routines/generated/users");
        for verb in ["list", "get", "create", "update", "delete"] {
            let p = users_dir.join(format!("{verb}.rs"));
            assert!(p.exists(), "missing {}", p.display());
            let body = stdfs::read_to_string(&p).expect("read");
            assert!(body.starts_with("// AUTO-GENERATED from "), "missing marker in {}", p.display());
        }

        let resource_barrel = stdfs::read_to_string(users_dir.join("mod.rs")).expect("read resource barrel");
        for v in ["list", "get", "create", "update", "delete"] {
            assert!(resource_barrel.contains(&format!("pub mod {v};")), "resource barrel missing {v}");
        }

        let top_barrel = stdfs::read_to_string(root.join("src/routines/generated/mod.rs")).expect("read top barrel");
        assert!(top_barrel.contains("pub mod users;"));
        assert!(top_barrel.starts_with("// AUTO-GENERATED from "));

        assert!(report.written.iter().any(|p| p.ends_with("mod.rs")));
    }

    #[test]
    fn empty_state_emits_nothing() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        stdfs::create_dir_all(root.join("storage/blast/state/resources")).expect("mkdir state");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("ok on empty");
        assert!(report.written.is_empty());
        assert!(!root.join("src/routines/generated").exists());
    }

    #[test]
    fn skips_resources_below_route_level() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resources_dir = root.join("storage/blast/state/resources");
        stdfs::create_dir_all(&resources_dir).expect("mkdir");

        let mut variants = BTreeSet::new();
        variants.insert(FieldVariant::Public);
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("BIGINT"),
                variants,
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        let resource = ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("draft"),
            fields,
            verbs: IndexMap::new(),
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: std::collections::BTreeMap::new(),
            gen_level: crate::state::GenLevel::Model,
        };
        stdfs::write(resources_dir.join("draft.ron"), ron::ser::to_string_pretty(&resource, ron::ser::PrettyConfig::default()).expect("ser")).expect("write");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("ok");
        assert!(report.written.is_empty());
        assert!(!root.join("src/routines/generated").exists());
    }
}
