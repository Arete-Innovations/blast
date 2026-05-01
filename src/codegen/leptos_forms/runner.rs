use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{
        enums::{scan::scan_project_enums, ParsedEnum},
        header, ir_loader,
        leptos_forms::render,
    },
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{GenLevel, ResourceState, Verb},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "leptos forms generation";
pub const FORM_EMIT_STRATEGY: &str = "hand-rolled thaw inputs + Action::new (no leptos-form derive); chosen for explicit validator-before-dispatch control + alignment with mutation UX rule (await server, no optimistic).";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);
    sink.info(format!("{STEP_LABEL}: strategy = {}", FORM_EMIT_STRATEGY));

    let all_resources = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };

    let enum_scan = match scan_project_enums(project_root) {
        Ok(rep) => rep,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };
    let enums: &[ParsedEnum] = &enum_scan.enums;

    let resources: Vec<ResourceState> = all_resources
        .into_iter()
        .filter(|r| r.gen_level >= GenLevel::Components)
        .filter(|r| has_form_verb(r))
        .collect();

    let forms_dir = forms_root_dir(project_root);
    let data_dir = data_root_dir(project_root);
    let components_generated_dir = components_generated_dir(project_root);

    fs::create_dir_all(&forms_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&components_generated_dir)?;

    let mut report = EmitReport::default();

    if resources.is_empty() {
        emit_empty_skeleton(project_root, &forms_dir, &data_dir, &components_generated_dir, &mut report)?;
        sink.info(format!("{STEP_LABEL}: no resources at gen_level >= Components with Create/Update verbs; emitted skeleton barrels"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let stub_owners: Vec<&ResourceState> = resources.iter().filter(|r| r.gen_level < GenLevel::Pages).collect();

    for r in &resources {
        emit_resource_forms(project_root, r, enums, &forms_dir, &mut report)?;
        sink.info(format!("emitted forms for {}", r.name.as_str()));
    }
    for r in &stub_owners {
        emit_resource_data_stub(project_root, r, &data_dir, &mut report)?;
    }

    let app_marker = header::marker_for_app(project_root)?;

    let table_names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();

    let forms_top_barrel_body = format!("{}{}", app_marker, render::render_top_forms_barrel(&table_names));
    write_file(&forms_dir.join("mod.rs"), &forms_top_barrel_body, &mut report)?;

    let components_generated_barrel_body = format!("{}{}", app_marker, render::render_components_generated_mod());
    write_file(&components_generated_dir.join("mod.rs"), &components_generated_barrel_body, &mut report)?;

    if !stub_owners.is_empty() {
        let union_tables = list_existing_data_tables(&data_dir);
        let union_table_strs: Vec<&str> = union_tables.iter().map(|s| s.as_str()).collect();
        let data_barrel_body = format!("{}{}", app_marker, render::render_data_barrel(&union_table_strs));
        write_file(&data_dir.join("mod.rs"), &data_barrel_body, &mut report)?;

        ensure_data_user_barrel_includes_generated(project_root, &mut report)?;
        ensure_leptos_user_barrel_includes_data(project_root, &mut report)?;
    }

    ensure_components_user_barrel(project_root, &mut report)?;

    sink.info(format!("{STEP_LABEL}: {} written, {} skipped", report.written.len(), report.skipped.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn has_form_verb(r: &ResourceState) -> bool {
    r.verbs.contains_key(&Verb::Create) || r.verbs.contains_key(&Verb::Update)
}

fn emit_resource_forms(project_root: &Path, resource: &ResourceState, enums: &[ParsedEnum], forms_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let table = resource.name.as_str();
    let resource_dir = forms_dir.join(table);
    fs::create_dir_all(&resource_dir)?;
    let marker = header::marker_for_resource(project_root, table)?;

    if resource.verbs.contains_key(&Verb::Create) {
        let body = format!("{}{}", marker, render::render_create_form(resource, enums));
        write_file(&resource_dir.join("create_form.rs"), &body, report)?;
    }
    if resource.verbs.contains_key(&Verb::Update) && render::primary_key_field(resource).is_some() {
        let body = format!("{}{}", marker, render::render_edit_form(resource, enums));
        write_file(&resource_dir.join("edit_form.rs"), &body, report)?;
    }

    let barrel_body = format!("{}{}", marker, render::render_resource_form_barrel(resource));
    write_file(&resource_dir.join("mod.rs"), &barrel_body, report)?;
    Ok(())
}

fn emit_resource_data_stub(project_root: &Path, resource: &ResourceState, data_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let table = resource.name.as_str();
    let marker = header::marker_for_resource(project_root, table)?;
    let body = format!("{}{}", marker, render::render_data_stub(resource));
    write_file(&data_dir.join(format!("{table}.rs")), &body, report)?;
    Ok(())
}

fn emit_empty_skeleton(project_root: &Path, forms_dir: &Path, _data_dir: &Path, components_generated_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let app_marker = header::marker_for_app(project_root)?;

    write_file(&forms_dir.join(".gitkeep"), "", report)?;

    let empty_forms_barrel = format!("{app_marker}\n");
    write_file(&forms_dir.join("mod.rs"), &empty_forms_barrel, report)?;

    let empty_components_generated = format!("{app_marker}{}", render::render_components_generated_mod());
    write_file(&components_generated_dir.join("mod.rs"), &empty_components_generated, report)?;
    Ok(())
}

fn ensure_components_user_barrel(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let user_barrel = project_root.join("src").join("transport").join("leptos").join("components").join("mod.rs");
    let existing = match fs::read_to_string(&user_barrel) {
        Ok(s) => s,
        Err(_e) => return Ok(()),
    };
    if existing.contains("\npub mod generated;\n") || existing.starts_with("pub mod generated;\n") || existing.ends_with("pub mod generated;\n") {
        return Ok(());
    }
    let updated = match existing.ends_with('\n') {
        true => format!("{existing}pub mod generated;\n"),
        false => format!("{existing}\npub mod generated;\n"),
    };
    fs::write(&user_barrel, &updated)?;
    report.written.push(user_barrel);
    Ok(())
}

fn ensure_data_user_barrel_includes_generated(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let user_barrel = project_root.join("src").join("transport").join("leptos").join("data").join("mod.rs");
    let body = match fs::read_to_string(&user_barrel) {
        Ok(prev) => {
            if prev.contains("pub mod generated;") {
                return Ok(());
            }
            match prev.ends_with('\n') {
                true => format!("{prev}pub mod generated;\n"),
                false => format!("{prev}\npub mod generated;\n"),
            }
        }
        Err(_io) => "pub mod generated;\n".to_string(),
    };
    let parent = user_barrel.parent().ok_or_else(|| BlastError::Invalid(format!("data barrel has no parent: {}", user_barrel.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(&user_barrel, &body)?;
    report.written.push(user_barrel);
    Ok(())
}

fn ensure_leptos_user_barrel_includes_data(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let user_barrel = project_root.join("src").join("transport").join("leptos").join("mod.rs");
    let existing = match fs::read_to_string(&user_barrel) {
        Ok(s) => s,
        Err(_e) => return Ok(()),
    };
    if existing.contains("\npub mod data;\n") || existing.starts_with("pub mod data;\n") || existing.ends_with("pub mod data;\n") {
        return Ok(());
    }
    let updated = match existing.ends_with('\n') {
        true => format!("{existing}pub mod data;\n"),
        false => format!("{existing}\npub mod data;\n"),
    };
    fs::write(&user_barrel, &updated)?;
    report.written.push(user_barrel);
    Ok(())
}

fn forms_root_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("components").join("generated").join("forms")
}

fn components_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("components").join("generated")
}

fn data_root_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("data").join("generated")
}

fn list_existing_data_tables(data_dir: &Path) -> Vec<String> {
    let mut tables: Vec<String> = Vec::new();
    let entries = match fs::read_dir(data_dir) {
        Ok(it) => it,
        Err(_io) => return tables,
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_io) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if file_name == "mod.rs" || file_name == ".gitkeep" {
            continue;
        }
        let stem = match file_name.strip_suffix(".rs") {
            Some(s) => s.to_string(),
            None => continue,
        };
        tables.push(stem);
    }
    tables.sort();
    tables
}

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("leptos_forms target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;

    let existing = read_existing(target)?;
    match existing {
        Some(prev) if prev == body => {
            report.skipped.push(target.to_path_buf());
            return Ok(());
        }
        Some(_different) => fs::write(target, body)?,
        None => fs::write(target, body)?,
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
            save_app, save_resource, AppState, GenLevel, SqlType,
        },
    };

    fn make_users_with_email_password(level: GenLevel) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let id_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin].into_iter().collect();
        let user_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();
        let secret_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch].into_iter().collect();

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
                variants: user_v,
                nullable: false,
                primary_key: false,
                validators: email_rules,
            },
        );
        let mut pw_rules: BTreeSet<ValidatorRule> = BTreeSet::new();
        pw_rules.insert(ValidatorRule::MinLen(8));
        fields.insert(
            FieldName::new("password"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: secret_v,
                nullable: false,
                primary_key: false,
                validators: pw_rules,
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
            gen_level: level,
        }
    }

    fn seed_project(root: &Path, resources: &[ResourceState]) {
        let state_dir = root.join("storage").join("blast").join("state");
        match save_app(&state_dir, &AppState::new()) {
            Ok(()) => {}
            Err(e) => panic!("save app failed: {e}"),
        }
        for r in resources {
            match save_resource(&state_dir, r) {
                Ok(()) => {}
                Err(e) => panic!("save resource failed: {e}"),
            }
        }
    }

    #[test]
    fn emits_create_and_edit_forms_for_users() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_users_with_email_password(GenLevel::Components);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = match run(root, &mut sink, &mut progress) {
            Ok(r) => r,
            Err(e) => panic!("run: {e}"),
        };

        let create_form = root.join("src/transport/leptos/components/generated/forms/users/create_form.rs");
        let edit_form = root.join("src/transport/leptos/components/generated/forms/users/edit_form.rs");
        let resource_barrel = root.join("src/transport/leptos/components/generated/forms/users/mod.rs");
        let top_forms_barrel = root.join("src/transport/leptos/components/generated/forms/mod.rs");
        let components_generated_barrel = root.join("src/transport/leptos/components/generated/mod.rs");
        let data_stub = root.join("src/transport/leptos/data/generated/users.rs");
        let data_barrel = root.join("src/transport/leptos/data/generated/mod.rs");

        assert!(create_form.exists(), "create_form.rs must exist");
        assert!(edit_form.exists(), "edit_form.rs must exist");
        assert!(resource_barrel.exists(), "per-resource mod.rs must exist");
        assert!(top_forms_barrel.exists(), "top forms mod.rs must exist");
        assert!(components_generated_barrel.exists(), "components/generated/mod.rs must exist");
        assert!(data_stub.exists(), "data stub must exist");
        assert!(data_barrel.exists(), "data barrel must exist");

        let written: Vec<&PathBuf> = report.written.iter().collect();
        assert!(written.iter().any(|p| *p == &create_form), "create_form must be reported written");
        assert!(written.iter().any(|p| *p == &edit_form), "edit_form must be reported written");
    }

    #[test]
    fn create_form_invokes_validator_and_data_helper() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_users_with_email_password(GenLevel::Components);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let body = match fs::read_to_string(root.join("src/transport/leptos/components/generated/forms/users/create_form.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read create_form: {e}"),
        };

        assert!(body.starts_with("// AUTO-GENERATED from "), "marker header expected");
        assert!(body.contains("validate_users_insertable("), "must call validator: {body}");
        assert!(body.contains("do_users_create("), "must reference data helper: {body}");
        assert!(body.contains("UserInsertable"), "must reference Insertable type: {body}");
        assert!(body.contains("UserCreateForm"), "component name expected: {body}");
        assert!(body.contains("on:submit=on_submit"), "form submit binding expected: {body}");
    }

    #[test]
    fn edit_form_invokes_patch_validator_and_update_helper() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_users_with_email_password(GenLevel::Components);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let body = match fs::read_to_string(root.join("src/transport/leptos/components/generated/forms/users/edit_form.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read edit_form: {e}"),
        };

        assert!(body.contains("validate_users_patch("), "must call patch validator: {body}");
        assert!(body.contains("do_users_update("), "must reference update helper: {body}");
        assert!(body.contains("UserPatch"), "must reference Patch type: {body}");
        assert!(body.contains("UserPublic"), "must reference Public type as initial prop: {body}");
        assert!(body.contains("UserEditForm"), "component name expected: {body}");
    }

    #[test]
    fn skips_resources_below_components_gen_level() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_users_with_email_password(GenLevel::Composables);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        assert!(!root.join("src/transport/leptos/components/generated/forms/users/create_form.rs").exists());
    }

    #[test]
    fn idempotent_second_run_skips_unchanged() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_users_with_email_password(GenLevel::Components);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _first = match run(root, &mut sink, &mut progress) {
            Ok(r) => r,
            Err(e) => panic!("first run: {e}"),
        };
        let second = match run(root, &mut sink, &mut progress) {
            Ok(r) => r,
            Err(e) => panic!("second run: {e}"),
        };

        assert!(second.written.is_empty(), "second run unexpectedly wrote: {:?}", second.written);
        assert!(!second.skipped.is_empty(), "second run must skip files");
    }

    #[test]
    fn no_resources_emits_skeleton_with_gitkeep() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        seed_project(root, &[]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        assert!(root.join("src/transport/leptos/components/generated/forms/.gitkeep").exists(), ".gitkeep expected when no qualifying resources");
        assert!(root.join("src/transport/leptos/components/generated/mod.rs").exists(), "components/generated/mod.rs expected");
        assert!(!root.join("src/transport/leptos/data/mod.rs").exists(), "user-owned data/mod.rs must NOT be written when no qualifying resources");
    }

    fn make_tasks_with_status_enum(level: GenLevel) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let id_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin].into_iter().collect();
        let status_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();

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
            FieldName::new("status"),
            FieldState {
                sql_type: SqlType::new("MyStatus"),
                variants: status_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in [Verb::Create, Verb::Update] {
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
            name: ResourceName::new("tasks"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: level,
        }
    }

    fn write_enum_migration(root: &Path) {
        let mig_dir = root.join("src/database/migrations/2026-05-01-000001_my_status");
        match fs::create_dir_all(&mig_dir) {
            Ok(()) => {}
            Err(e) => panic!("mkdir migration: {e}"),
        }
        let body = "CREATE TYPE my_status AS ENUM ('pending', 'active', 'done');\n";
        match fs::write(mig_dir.join("up.sql"), body) {
            Ok(()) => {}
            Err(e) => panic!("write up.sql: {e}"),
        }
    }

    #[test]
    fn enum_field_renders_combobox_with_variants() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_tasks_with_status_enum(GenLevel::Components);
        seed_project(root, &[resource]);
        write_enum_migration(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let create_body = match fs::read_to_string(root.join("src/transport/leptos/components/generated/forms/tasks/create_form.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read create_form: {e}"),
        };

        assert!(create_body.contains("use thaw::{Checkbox, Combobox, ComboboxOption, Input, InputType, Textarea};"), "thaw imports must include Combobox + ComboboxOption: {create_body}");
        assert!(create_body.contains("use crate::structs::generated::enums::MyStatus;"), "must import the MyStatus enum: {create_body}");
        assert!(create_body.contains("<Combobox value=status>"), "status field must be rendered as Combobox: {create_body}");
        assert!(create_body.contains("<ComboboxOption value=\"pending\".to_string() text=\"pending\".to_string()/>"), "must list 'pending' variant: {create_body}");
        assert!(create_body.contains("<ComboboxOption value=\"active\".to_string() text=\"active\".to_string()/>"), "must list 'active' variant: {create_body}");
        assert!(create_body.contains("<ComboboxOption value=\"done\".to_string() text=\"done\".to_string()/>"), "must list 'done' variant: {create_body}");
        assert!(create_body.contains("MyStatus::parse(&status_raw)"), "Action must call MyStatus::parse on the raw signal value: {create_body}");
        assert!(create_body.contains("MeltDown::validation_failed_field(\"status\", \"invalid MyStatus\")"), "parse failure must propagate as validation_failed_field with field+enum-name message: {create_body}");

        let edit_body = match fs::read_to_string(root.join("src/transport/leptos/components/generated/forms/tasks/edit_form.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read edit_form: {e}"),
        };

        assert!(edit_body.contains("use thaw::{Checkbox, Combobox, ComboboxOption, Input, InputType, Textarea};"), "edit form thaw imports must include Combobox: {edit_body}");
        assert!(edit_body.contains("<Combobox value=status>"), "edit form must also use Combobox for enum field: {edit_body}");
        assert!(edit_body.contains("MyStatus::parse(&status_raw)"), "edit form must parse the enum from the raw signal value: {edit_body}");
    }
}
