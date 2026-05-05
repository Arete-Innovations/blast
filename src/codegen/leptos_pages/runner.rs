use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{
        header, ir_loader,
        leptos_pages::render::{render_create_page, render_detail_page, render_edit_page, render_list_page},
        structs::naming::type_stem_for_resource,
    },
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{AuthMode, GenLevel, ResourceState, Verb},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "leptos pages generation";

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

    let pages_dir = pages_generated_dir(project_root);
    let data_dir = data_generated_dir(project_root);
    fs::create_dir_all(&pages_dir)?;
    fs::create_dir_all(&data_dir)?;

    let mut report = EmitReport::default();

    if resources.is_empty() {
        let pages_keep = pages_dir.join(".gitkeep");
        write_file(&pages_keep, "", &mut report)?;
        let data_keep = data_dir.join(".gitkeep");
        write_file(&data_keep, "", &mut report)?;

        let app_marker = header::marker_for_app(project_root)?;
        let pages_barrel = pages_dir.join("mod.rs");
        let pages_barrel_body = format!("{app_marker}\n");
        write_file(&pages_barrel, &pages_barrel_body, &mut report)?;
        let data_barrel = data_dir.join("mod.rs");
        let data_barrel_body = format!("{app_marker}\n");
        write_file(&data_barrel, &data_barrel_body, &mut report)?;

        sink.info(format!("{STEP_LABEL}: no resources at gen_level >= Pages; emitted barrels"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let mut emitted_tables: Vec<String> = Vec::with_capacity(resources.len());
    for r in &resources {
        emit_resource(project_root, r, &pages_dir, &data_dir, &mut report)?;
        emitted_tables.push(r.name.as_str().to_string());
        sink.info(format!("emitted leptos pages for {}", r.name.as_str()));
    }
    emitted_tables.sort();

    let app_marker = header::marker_for_app(project_root)?;
    let pages_barrel = pages_dir.join("mod.rs");
    let pages_barrel_body = format!("{app_marker}{}", build_top_barrel(&emitted_tables));
    write_file(&pages_barrel, &pages_barrel_body, &mut report)?;

    let data_tables = list_existing_data_tables(&data_dir);
    let data_barrel = data_dir.join("mod.rs");
    let data_barrel_body = format!("{app_marker}{}", build_top_barrel(&data_tables));
    write_file(&data_barrel, &data_barrel_body, &mut report)?;

    ensure_parent_pages_barrel(project_root, &mut report)?;
    ensure_parent_data_barrel(project_root, &mut report)?;
    ensure_leptos_mod_includes_data(project_root, &mut report)?;

    sink.info(format!("{STEP_LABEL}: {} written, {} skipped", report.written.len(), report.skipped.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn pages_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("pages").join("generated")
}

fn data_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("data").join("generated")
}

fn emit_resource(project_root: &Path, resource: &ResourceState, pages_dir: &Path, data_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let table = resource.name.as_str();
    let marker = header::marker_for_resource(project_root, table)?;

    let resource_pages_dir = pages_dir.join(table);
    fs::create_dir_all(&resource_pages_dir)?;

    let stem = type_stem_for_resource(resource);

    let mut emitted_modules: Vec<&'static str> = Vec::new();

    if verb_emits_html(resource, Verb::List) {
        let body = render_list_page(resource, &stem, verb_auth(resource, Verb::List)?);
        write_file(&resource_pages_dir.join("list.rs"), &format!("{marker}{body}"), report)?;
        emitted_modules.push("list");
    }
    if verb_emits_html(resource, Verb::Get) {
        let body = render_detail_page(resource, &stem, verb_auth(resource, Verb::Get)?);
        write_file(&resource_pages_dir.join("detail.rs"), &format!("{marker}{body}"), report)?;
        emitted_modules.push("detail");
    }
    if verb_emits_html(resource, Verb::Create) {
        let body = render_create_page(table, &stem, verb_auth(resource, Verb::Create)?);
        write_file(&resource_pages_dir.join("create.rs"), &format!("{marker}{body}"), report)?;
        emitted_modules.push("create");
    }
    if verb_emits_html(resource, Verb::Update) {
        let body = render_edit_page(table, &stem, verb_auth(resource, Verb::Update)?);
        write_file(&resource_pages_dir.join("edit.rs"), &format!("{marker}{body}"), report)?;
        emitted_modules.push("edit");
    }

    let resource_barrel = resource_pages_dir.join("mod.rs");
    let resource_barrel_body = format!("{marker}{}", build_resource_pages_barrel(&emitted_modules, &stem));
    write_file(&resource_barrel, &resource_barrel_body, report)?;

    emit_data_stub(project_root, resource, data_dir, report)?;

    Ok(())
}

fn verb_emits_html(resource: &ResourceState, verb: Verb) -> bool {
    match resource.verbs.get(&verb) {
        Some(state) => state.emit_html_page,
        None => false, // allow: absent verb declaration means no HTML page emission for this verb
    }
}

fn verb_auth(resource: &ResourceState, verb: Verb) -> BlastResult<AuthMode> {
    match resource.verbs.get(&verb) {
        Some(state) => Ok(state.auth.clone()),
        None => Err(BlastError::Invalid(format!("verb {:?} vanished from resource {} between iter and lookup", verb, resource.name.as_str()))),
    }
}

fn build_resource_pages_barrel(modules: &[&'static str], stem: &str) -> String {
    let mut out = String::new();
    let mut sorted: Vec<&&'static str> = modules.iter().collect();
    sorted.sort();
    for m in &sorted {
        out.push_str(&format!("pub mod {};\n", m));
    }
    if !sorted.is_empty() {
        out.push('\n');
    }
    for m in &sorted {
        let component_suffix = match **m {
            "list" => "ListPage",
            "detail" => "DetailPage",
            "create" => "CreatePage",
            "edit" => "EditPage",
            _other => continue,
        };
        out.push_str(&format!("pub use {m}::{stem}{component_suffix};\n"));
    }
    out
}

fn build_top_barrel(tables: &[String]) -> String {
    let mut out = String::new();
    for t in tables {
        out.push_str(&format!("pub mod {t};\n"));
    }
    out
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

fn emit_data_stub(project_root: &Path, resource: &ResourceState, data_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let table = resource.name.as_str();
    let marker = header::marker_for_resource(project_root, table)?;
    let stem = type_stem_for_resource(resource);
    let body = render_data_stub(table, &stem, resource);
    let path = data_dir.join(format!("{table}.rs"));
    write_file(&path, &format!("{marker}{body}"), report)?;
    Ok(())
}

fn render_data_stub(table: &str, stem: &str, resource: &ResourceState) -> String {
    let public_ty = format!("{stem}Public");
    let insertable_ty = format!("{stem}Insertable");
    let patch_ty = format!("{stem}Patch");

    let mut out = String::new();
    out.push_str("use crate::meltdown::{MeltDown, MeltType};\n");
    out.push_str(&format!("use crate::structs::generated::{table}::*;\n\n"));

    let stub_err = format!("|| MeltDown::new(MeltType::Unexpected(\"data helper not implemented for {table}\".to_string()), \"data helper stub\")");

    if resource.verbs.contains_key(&Verb::List) {
        out.push_str(&format!(
            "pub async fn load_{table}_list() -> Result<Vec<{public_ty}>, MeltDown> {{\n\
             \x20   Err(({stub_err})())\n\
             }}\n\n",
        ));
    }
    if resource.verbs.contains_key(&Verb::Get) {
        out.push_str(&format!(
            "pub async fn load_{table}_one(_id: i64) -> Result<{public_ty}, MeltDown> {{\n\
             \x20   Err(({stub_err})())\n\
             }}\n\n",
        ));
    }
    if resource.verbs.contains_key(&Verb::Create) {
        out.push_str(&format!(
            "pub async fn do_{table}_create(_input: {insertable_ty}) -> Result<{public_ty}, MeltDown> {{\n\
             \x20   Err(({stub_err})())\n\
             }}\n\n",
        ));
    }
    if resource.verbs.contains_key(&Verb::Update) {
        out.push_str(&format!(
            "pub async fn do_{table}_update(_id: i64, _input: {patch_ty}) -> Result<{public_ty}, MeltDown> {{\n\
             \x20   Err(({stub_err})())\n\
             }}\n\n",
        ));
    }
    if resource.verbs.contains_key(&Verb::Delete) {
        out.push_str(&format!(
            "pub async fn do_{table}_delete(_id: i64) -> Result<(), MeltDown> {{\n\
             \x20   Err(({stub_err})())\n\
             }}\n",
        ));
    }
    out
}

fn ensure_parent_pages_barrel(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let parent_barrel = project_root.join("src").join("transport").join("leptos").join("pages").join("mod.rs");
    let existing = match fs::read_to_string(&parent_barrel) {
        Ok(s) => s,
        Err(_io_err) => return Ok(()),
    };
    if existing.contains("pub mod generated;") {
        return Ok(());
    }
    let updated = if existing.ends_with('\n') {
        format!("{existing}\npub mod generated;\n")
    } else {
        format!("{existing}\n\npub mod generated;\n")
    };
    fs::write(&parent_barrel, &updated)?;
    report.written.push(parent_barrel);
    Ok(())
}

fn ensure_parent_data_barrel(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let parent_barrel = project_root.join("src").join("transport").join("leptos").join("data").join("mod.rs");
    let existing = fs::read_to_string(&parent_barrel);
    let body = match existing {
        Ok(prev) => {
            if prev.contains("pub mod generated;") {
                return Ok(());
            }
            if prev.ends_with('\n') {
                format!("{prev}\npub mod generated;\n")
            } else {
                format!("{prev}\n\npub mod generated;\n")
            }
        }
        Err(_io_err) => "pub mod generated;\n".to_string(),
    };
    fs::create_dir_all(parent_barrel.parent().ok_or_else(|| BlastError::Invalid(format!("data parent has no dir: {}", parent_barrel.display())))?)?;
    fs::write(&parent_barrel, &body)?;
    report.written.push(parent_barrel);
    Ok(())
}

fn ensure_leptos_mod_includes_data(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let mod_path = project_root.join("src").join("transport").join("leptos").join("mod.rs");
    let existing = match fs::read_to_string(&mod_path) {
        Ok(s) => s,
        Err(_io_err) => return Ok(()),
    };
    if existing.contains("pub mod data;") {
        return Ok(());
    }
    let updated = if existing.ends_with('\n') {
        format!("{existing}pub mod data;\n")
    } else {
        format!("{existing}\npub mod data;\n")
    };
    fs::write(&mod_path, &updated)?;
    report.written.push(mod_path);
    Ok(())
}

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("leptos pages target has no parent: {}", target.display())))?;
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
            resource::{AuthMode, FieldState, FieldVariant, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION},
            save_app, save_resource, AppState, SqlType,
        },
    };

    fn make_posts_with_all_verbs() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let id_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public, FieldVariant::Admin].into_iter().collect();
        let body_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public, FieldVariant::Admin].into_iter().collect();

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: id_v,
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            
            kind: Default::default(),
        },
        );
        fields.insert(
            FieldName::new("title"),
            FieldState {
                sql_type: SqlType::new("Text"),
                variants: body_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            
            kind: Default::default(),
        },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::AdminOnly,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );
        verbs.insert(
            Verb::Delete,
            VerbState {
                auth: AuthMode::AdminOnly,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("posts"),
            fields,
            verbs,
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

    fn seed_project(root: &Path, resources: &[ResourceState]) {
        let state_dir = root.join("storage").join("blast").join("state");
        save_app(&state_dir, &AppState::new()).expect("save app");
        for r in resources {
            save_resource(&state_dir, r).expect("save resource");
        }
    }

    #[test]
    fn emits_all_four_pages_for_full_verb_set() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let base = root.join("src/transport/leptos/pages/generated/posts");
        let list = base.join("list.rs");
        let detail = base.join("detail.rs");
        let create = base.join("create.rs");
        let edit = base.join("edit.rs");
        let resource_barrel = base.join("mod.rs");
        let top_barrel = root.join("src/transport/leptos/pages/generated/mod.rs");

        assert!(list.exists(), "list.rs must exist");
        assert!(detail.exists(), "detail.rs must exist");
        assert!(create.exists(), "create.rs must exist");
        assert!(edit.exists(), "edit.rs must exist");
        assert!(resource_barrel.exists(), "per-resource mod.rs must exist");
        assert!(top_barrel.exists(), "top-level pages/generated/mod.rs must exist");

        for path in [&list, &detail, &create, &edit, &resource_barrel, &top_barrel] {
            assert!(report.written.iter().any(|p| p == path), "report must include {}", path.display());
        }
    }

    #[test]
    fn pages_wrap_in_app_shell_under_bleed_layout() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let list_body = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/list.rs")).expect("read list");
        let detail_body = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/detail.rs")).expect("read detail");
        let create_body = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/create.rs")).expect("read create");
        let edit_body = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/edit.rs")).expect("read edit");

        for (label, body) in [("list", &list_body), ("detail", &detail_body), ("create", &create_body), ("edit", &edit_body)] {
            assert!(body.contains("PageLayout::Bleed"), "{label} page must use Bleed layout");
            assert!(body.contains("crud-toolbar"), "{label} page must include crud-toolbar chrome");
        }
        assert!(list_body.contains("<PublicShell"), "Public-list page must wrap in PublicShell");
        assert!(!list_body.contains("<Breadcrumb"), "Public-list page must drop admin breadcrumb");
        for (label, body) in [("detail", &detail_body), ("create", &create_body), ("edit", &edit_body)] {
            assert!(body.contains("<AppShell"), "{label} page must wrap in AppShell");
            assert!(body.contains("<Breadcrumb"), "{label} page must render Breadcrumb");
        }

        assert!(list_body.contains("TableBuilder::new"), "list page uses TableBuilder");
        assert!(list_body.contains("<Pagination"), "list page renders Pagination");
        assert!(list_body.contains("<EmptyState"), "list page wires EmptyState fallback");
        assert!(detail_body.contains("DetailBuilder::new"), "detail page uses DetailBuilder");
        assert!(detail_body.contains("ConfirmDialog"), "detail page wires ConfirmDialog when delete present");
    }

    #[test]
    fn auth_mode_maps_correctly_per_verb() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let list_body = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/list.rs")).expect("read list");
        let create_body = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/create.rs")).expect("read create");
        let edit_body = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/edit.rs")).expect("read edit");

        assert!(list_body.contains("AuthGuardMode::Public"), "list (Public auth) must map to AuthGuardMode::Public");
        assert!(create_body.contains("AuthGuardMode::Required"), "create (AuthRequired) must map to AuthGuardMode::Required");
        assert!(edit_body.contains("AuthGuardMode::AdminOnly"), "edit (AdminOnly) must map to AuthGuardMode::AdminOnly");
    }

    #[test]
    fn marker_header_present_on_each_emitted_file() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let pages = ["list.rs", "detail.rs", "create.rs", "edit.rs", "mod.rs"];
        for page in pages {
            let path = root.join(format!("src/transport/leptos/pages/generated/posts/{page}"));
            let body = fs::read_to_string(&path).expect("read page");
            assert!(!body.starts_with("// AUTO-GENERATED"), "no inline marker — use codegen.lock.ron sidecar");
        }
    }

    #[test]
    fn emit_html_page_false_skips_page_emission() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let mut resource = make_posts_with_all_verbs();
        let create = match resource.verbs.get_mut(&Verb::Create) {
            Some(v) => v,
            None => panic!("fixture missing Create"),
        };
        create.emit_html_page = false;
        seed_project(root, &[resource]);
        run(root, &mut NullSink, &mut NullProgress).expect("run leptos_pages");
        let base = root.join("src/transport/leptos/pages/generated/posts");
        assert!(base.join("list.rs").exists(), "list.rs must emit");
        assert!(!base.join("create.rs").exists(), "create.rs must NOT emit");
    }

    #[test]
    fn skips_resources_below_pages_gen_level() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let mut resource = make_posts_with_all_verbs();
        resource.gen_level = GenLevel::Components;
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let posts_dir = root.join("src/transport/leptos/pages/generated/posts");
        assert!(!posts_dir.exists(), "must NOT emit posts/ when gen_level < Pages");
    }

    #[test]
    fn idempotent_second_run_skips_unchanged_files() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _first = run(root, &mut sink, &mut progress).expect("first run");
        let second = run(root, &mut sink, &mut progress).expect("second run");

        assert!(!second.skipped.is_empty(), "second run must skip unchanged files");
    }

    #[test]
    fn no_resources_emits_gitkeep_and_empty_barrel() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        seed_project(root, &[]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let pages_keep = root.join("src/transport/leptos/pages/generated/.gitkeep");
        let data_keep = root.join("src/transport/leptos/data/generated/.gitkeep");
        let pages_barrel = root.join("src/transport/leptos/pages/generated/mod.rs");
        let data_barrel = root.join("src/transport/leptos/data/generated/mod.rs");
        assert!(pages_keep.exists(), ".gitkeep must exist for pages when no resources");
        assert!(data_keep.exists(), ".gitkeep must exist for data when no resources");
        assert!(pages_barrel.exists(), "empty pages barrel must exist");
        assert!(data_barrel.exists(), "empty data barrel must exist");
    }

    #[test]
    fn data_stub_emits_per_verb_helpers() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let data = fs::read_to_string(root.join("src/transport/leptos/data/generated/posts.rs")).expect("read data stub");
        assert!(data.contains("pub async fn load_posts_list"), "missing load_posts_list");
        assert!(data.contains("pub async fn load_posts_one"), "missing load_posts_one");
        assert!(data.contains("pub async fn do_posts_create"), "missing do_posts_create");
        assert!(data.contains("pub async fn do_posts_update"), "missing do_posts_update");
        assert!(data.contains("pub async fn do_posts_delete"), "missing do_posts_delete");
    }

    #[test]
    fn top_barrel_lists_emitted_resources() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let top_barrel = fs::read_to_string(root.join("src/transport/leptos/pages/generated/mod.rs")).expect("read barrel");
        assert!(top_barrel.contains("pub mod posts;"), "top barrel must list posts: {top_barrel}");
    }

    #[test]
    fn detail_page_extracts_id_from_route_params() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let detail = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/detail.rs")).expect("read detail");
        assert!(detail.contains("use_params_map"), "detail must read params from router: {detail}");
        assert!(detail.contains("id_signal"), "detail must declare id_signal: {detail}");
        assert!(detail.contains("load_posts_one(id)"), "detail must call loader with id from signal, not hardcoded: {detail}");
        assert!(!detail.contains("load_posts_one(0)"), "detail must NOT hardcode id=0: {detail}");
    }

    #[test]
    fn detail_page_emits_delete_button_with_id_when_delete_verb_present() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let detail = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/detail.rs")).expect("read detail");
        assert!(detail.contains("do_posts_delete(id)"), "detail must call deleter with id from signal: {detail}");
        assert!(detail.contains("on_delete"), "detail must wire on_delete handler: {detail}");
    }

    #[test]
    fn edit_page_extracts_id_from_route_params() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        let resource = make_posts_with_all_verbs();
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run leptos_pages");

        let edit = fs::read_to_string(root.join("src/transport/leptos/pages/generated/posts/edit.rs")).expect("read edit");
        assert!(edit.contains("use_params_map"), "edit must read params from router: {edit}");
        assert!(edit.contains("id_signal"), "edit must declare id_signal: {edit}");
        assert!(edit.contains("load_posts_one(id)"), "edit must call loader with id from signal, not hardcoded: {edit}");
        assert!(!edit.contains("load_posts_one(0)"), "edit must NOT hardcode id=0: {edit}");
    }
}
