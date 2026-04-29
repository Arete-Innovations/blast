use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{composables::render::build_resource_composables, header, ir_loader, structs::naming::type_stem_for_resource},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{GenLevel, ResourceState, Verb},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "frontend composables generation";

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

    let resources: Vec<_> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Composables).collect();

    let out_dir = composables_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let mut report = EmitReport::default();

    if resources.is_empty() {
        let gitkeep = out_dir.join(".gitkeep");
        write_file(&gitkeep, "", &mut report)?;
        sink.info(format!("{STEP_LABEL}: no resources declared; emitted .gitkeep"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    for r in &resources {
        let table = r.name.as_str();
        let marker = header::marker_for_resource(project_root, table)?;
        let body = format!("{marker}{}", build_resource_composables(r));
        let path = out_dir.join(format!("{table}.ts"));
        write_file(&path, &body, &mut report)?;
        sink.info(format!("emitted {}", path.display()));
    }

    let barrel_path = out_dir.join("index.ts");
    let barrel_body = build_index_barrel(&resources);
    write_file(&barrel_path, &barrel_body, &mut report)?;
    sink.info(format!("emitted {}", barrel_path.display()));

    sink.info(format!("{STEP_LABEL}: {} written, {} skipped", report.written.len(), report.skipped.len()));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

pub fn run_for_resource(project_root: &Path, resource_name: &str, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    let all = ir_loader::load_resource_states(project_root)?;
    let filtered: Vec<ResourceState> = all.into_iter().filter(|r| r.name.as_str() == resource_name && r.gen_level >= GenLevel::Composables).collect();
    if filtered.is_empty() {
        sink.warn(format!("no resource named '{resource_name}' at gen_level >= Composables"));
        return Ok(EmitReport::default());
    }
    progress.step_start(STEP_LABEL);

    let out_dir = composables_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let mut report = EmitReport::default();
    for r in &filtered {
        let table = r.name.as_str();
        let marker = header::marker_for_resource(project_root, table)?;
        let body = format!("{marker}{}", build_resource_composables(r));
        let path = out_dir.join(format!("{table}.ts"));
        write_file(&path, &body, &mut report)?;
        sink.info(format!("emitted {}", path.display()));
    }
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn composables_dir(project_root: &Path) -> PathBuf {
    project_root.join("frontend").join("src").join("generated").join("composables")
}

fn build_index_barrel(resources: &[ResourceState]) -> String {
    let mut sorted: Vec<&ResourceState> = resources.iter().collect();
    sorted.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));

    let mut out = String::new();
    for r in &sorted {
        let table = r.name.as_str();
        let singular = type_stem_for_resource(r);
        let mut symbols: Vec<String> = Vec::new();
        if r.verbs.contains_key(&Verb::List) {
            symbols.push(format!("use{}List", plural_of_pascal(&singular, table)));
        }
        if r.verbs.contains_key(&Verb::Get) {
            symbols.push(format!("use{}", singular));
        }
        if r.verbs.contains_key(&Verb::Create) {
            symbols.push(format!("useCreate{}", singular));
        }
        if r.verbs.contains_key(&Verb::Update) {
            symbols.push(format!("useUpdate{}", singular));
        }
        if r.verbs.contains_key(&Verb::Delete) {
            symbols.push(format!("useDelete{}", singular));
        }
        if symbols.is_empty() {
            continue;
        }
        out.push_str(&format!("export {{ {names} }} from './{table}'\n", names = symbols.join(", "), table = table));
    }
    if out.is_empty() {
        out.push_str("export {}\n");
    }
    out
}

fn plural_of_pascal(singular: &str, table: &str) -> String {
    let mut out = String::with_capacity(table.len());
    let mut upper_next = true;
    for ch in table.chars() {
        if ch == '_' || ch == '-' {
            upper_next = true;
            continue;
        }
        if upper_next {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    if out.starts_with(singular) {
        out
    } else {
        format!("{singular}s")
    }
}

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("composables target has no parent: {}", target.display())))?;
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
            resource::{AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION},
            save_app, save_resource, AppState, SqlType,
        },
    };

    fn make_users_resource() -> ResourceState {
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
        for v in [Verb::List, Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
            let list_opts = match v {
                Verb::List => Some(ListOptions {
                    paginated: true,
                    filterable_columns: BTreeMap::new(),
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: None,
                }),
                _other => None,
            };
            verbs.insert(
                v,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: list_opts,
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
    fn emits_gitkeep_when_no_resources() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_project(root, &[]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run composables");

        let gitkeep = root.join("frontend/src/generated/composables/.gitkeep");
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
        let report = run(root, &mut sink, &mut progress).expect("run composables");

        let composables_file = root.join("frontend/src/generated/composables/users.ts");
        assert!(composables_file.exists(), "users.ts must exist");
        let body = fs::read_to_string(&composables_file).expect("read users.ts");
        assert!(body.starts_with("// AUTO-GENERATED from "), "marker required");
        assert!(body.contains("export function useUsersList"), "useUsersList required");
        assert!(body.contains("export function useUser"), "useUser required");
        assert!(body.contains("export function useCreateUser"), "useCreateUser required");
        assert!(body.contains("export function useUpdateUser"), "useUpdateUser required");
        assert!(body.contains("export function useDeleteUser"), "useDeleteUser required");
        assert!(report.written.iter().any(|p| p == &composables_file));
    }

    #[test]
    fn emits_index_barrel() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_users_resource();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run composables");

        let barrel = root.join("frontend/src/generated/composables/index.ts");
        assert!(barrel.exists(), "index.ts barrel must exist");
        let body = fs::read_to_string(&barrel).expect("read index.ts");
        assert!(body.contains("from './users'"));
        assert!(body.contains("useUsersList"));
        assert!(body.contains("useDeleteUser"));
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
    fn skips_resources_below_composables_gen_level() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let mut resource = make_users_resource();
        resource.gen_level = crate::state::GenLevel::Types;
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run composables");

        let composables_file = root.join("frontend/src/generated/composables/users.ts");
        assert!(!composables_file.exists(), "users.ts must NOT exist when gen_level < Composables");
        let gitkeep = root.join("frontend/src/generated/composables/.gitkeep");
        assert!(gitkeep.exists(), ".gitkeep must exist as fallback");
    }
}
