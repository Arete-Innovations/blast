use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader, validators::render::build_resource_validators_rust},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{GenLevel, ResourceState},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "validators generation";

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

    let resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Types).collect();

    let rust_dir = rust_validators_dir(project_root);
    fs::create_dir_all(&rust_dir)?;

    let mut report = EmitReport::default();

    if resources.is_empty() {
        let rust_keep = rust_dir.join(".gitkeep");
        write_file(&rust_keep, "", &mut report)?;
        let rust_barrel = rust_dir.join("mod.rs");
        let app_marker = header::marker_for_app(project_root)?;
        let empty_barrel = format!("{app_marker}\n");
        write_file(&rust_barrel, &empty_barrel, &mut report)?;
        sink.info(format!("{STEP_LABEL}: no resources at gen_level >= Types; emitted barrels"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    for r in &resources {
        let table = r.name.as_str();
        let marker = header::marker_for_resource(project_root, table)?;

        let rust_body = format!("{marker}{}", build_resource_validators_rust(r));
        let rust_path = rust_dir.join(format!("{table}.rs"));
        write_file(&rust_path, &rust_body, &mut report)?;
        sink.info(format!("emitted {}", rust_path.display()));
    }

    let app_marker = header::marker_for_app(project_root)?;
    let rust_barrel = rust_dir.join("mod.rs");
    let rust_barrel_body = format!("{app_marker}{}", build_rust_barrel(&resources));
    write_file(&rust_barrel, &rust_barrel_body, &mut report)?;
    sink.info(format!("emitted {}", rust_barrel.display()));

    ensure_parent_structs_barrel_includes_validators(project_root, &mut report)?;

    sink.info(format!("{STEP_LABEL}: {} written, {} skipped", report.written.len(), report.skipped.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn ensure_parent_structs_barrel_includes_validators(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let parent_barrel = project_root.join("src").join("structs").join("generated").join("mod.rs");
    let existing = match fs::read_to_string(&parent_barrel) {
        Ok(s) => s,
        Err(_e) => return Ok(()),
    };
    if existing.contains("\npub mod validators;\n") || existing.ends_with("pub mod validators;\n") {
        return Ok(());
    }
    let updated = if existing.ends_with('\n') {
        format!("{existing}\npub mod validators;\n")
    } else {
        format!("{existing}\n\npub mod validators;\n")
    };
    fs::write(&parent_barrel, &updated)?;
    report.written.push(parent_barrel);
    Ok(())
}

pub fn run_for_resource(project_root: &Path, resource_name: &str, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    let all = ir_loader::load_resource_states(project_root)?;
    let filtered: Vec<ResourceState> = all.into_iter().filter(|r| r.name.as_str() == resource_name && r.gen_level >= GenLevel::Types).collect();
    if filtered.is_empty() {
        sink.warn(format!("no resource named '{resource_name}' at gen_level >= Types"));
        return Ok(EmitReport::default());
    }
    progress.step_start(STEP_LABEL);

    let rust_dir = rust_validators_dir(project_root);
    fs::create_dir_all(&rust_dir)?;

    let mut report = EmitReport::default();
    for r in &filtered {
        let table = r.name.as_str();
        let marker = header::marker_for_resource(project_root, table)?;

        let rust_body = format!("{marker}{}", build_resource_validators_rust(r));
        let rust_path = rust_dir.join(format!("{table}.rs"));
        write_file(&rust_path, &rust_body, &mut report)?;
        sink.info(format!("emitted {}", rust_path.display()));
    }

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn rust_validators_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("structs").join("generated").join("validators")
}

fn build_rust_barrel(resources: &[ResourceState]) -> String {
    let mut names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    names.sort();
    let mut out = String::new();
    for name in &names {
        out.push_str(&format!("pub mod {name};\n"));
    }
    out
}

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("validators target has no parent: {}", target.display())))?;
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
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        io::null::{NullProgress, NullSink},
        state::{
            names::{FieldName, ResourceName},
            resource::{AuthMode, FieldState, FieldVariant, ResourceState, ValidatorRule, Verb, VerbState, RESOURCE_SCHEMA_VERSION},
            save_app, save_resource, AppState, SqlType,
        },
    };

    fn make_users_with_email() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();
        let id_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public].into_iter().collect();

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
        let mut email_rules: BTreeSet<ValidatorRule> = BTreeSet::new();
        email_rules.insert(ValidatorRule::Email);
        email_rules.insert(ValidatorRule::MaxLen(254));
        fields.insert(
            FieldName::new("email"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: email_rules,
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in [Verb::Create, Verb::Update] {
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

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("users"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: crate::state::GenLevel::default(),
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
    fn emits_per_resource_files() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_with_email();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run validators");

        let rust_file = root.join("src/structs/generated/validators/users.rs");
        assert!(rust_file.exists(), "rust validator file must exist");
        assert!(report.written.iter().any(|p| p == &rust_file));
    }

    #[test]
    fn emits_barrel() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_with_email();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run validators");

        let rust_barrel = root.join("src/structs/generated/validators/mod.rs");
        assert!(rust_barrel.exists(), "rust barrel must exist");

        let rust_body = fs::read_to_string(&rust_barrel).expect("read rust barrel");
        assert!(rust_body.contains("pub mod users;"));
    }

    #[test]
    fn emitted_files_carry_marker_header() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_with_email();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run validators");

        let rust_file = root.join("src/structs/generated/validators/users.rs");
        let rust_body = fs::read_to_string(&rust_file).expect("read rust");
        assert!(rust_body.starts_with("// AUTO-GENERATED from "));
    }

    #[test]
    fn skips_resources_below_types_gen_level() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let mut resource = make_users_with_email();
        resource.gen_level = crate::state::GenLevel::Route;
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run validators");

        let rust_file = root.join("src/structs/generated/validators/users.rs");
        assert!(!rust_file.exists(), "must NOT emit when gen_level < Types");
    }

    #[test]
    fn idempotent_second_run_skips_unchanged() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_with_email();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _first = run(root, &mut sink, &mut progress).expect("first run");
        let second = run(root, &mut sink, &mut progress).expect("second run");

        assert!(second.written.is_empty(), "second run wrote {:?}", second.written);
        assert!(!second.skipped.is_empty(), "second run must skip files");
    }

    #[test]
    fn no_resources_emits_gitkeep_and_empty_barrel() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_project(root, &[]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run validators");

        let rust_keep = root.join("src/structs/generated/validators/.gitkeep");
        assert!(rust_keep.exists(), ".gitkeep must exist when no resources");
    }
}
