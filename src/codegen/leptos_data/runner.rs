use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader, leptos_data::render},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{GenLevel, ResourceState},
};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "leptos data generation";

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

    let resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Composables).collect();

    let data_dir = data_generated_dir(project_root);
    fs::create_dir_all(&data_dir)?;

    let mut report = EmitReport::default();

    if resources.is_empty() {
        let keep = data_dir.join(".gitkeep");
        write_file(&keep, "", &mut report)?;
        let app_marker = header::marker_for_app(project_root)?;
        let empty_barrel = format!("{app_marker}\n");
        write_file(&data_dir.join("mod.rs"), &empty_barrel, &mut report)?;
        sink.info(format!("{STEP_LABEL}: no resources at gen_level >= Composables; emitted barrels"));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let mut emitted_tables: Vec<String> = Vec::with_capacity(resources.len());
    for r in &resources {
        emit_resource(project_root, r, &data_dir, &mut report)?;
        emitted_tables.push(r.name.as_str().to_string());
        sink.info(format!("emitted leptos data helpers for {}", r.name.as_str()));
    }
    emitted_tables.sort();

    let app_marker = header::marker_for_app(project_root)?;
    let table_strs: Vec<&str> = emitted_tables.iter().map(|s| s.as_str()).collect();
    let barrel_body = format!("{app_marker}{}", render::render_top_data_barrel(&table_strs));
    write_file(&data_dir.join("mod.rs"), &barrel_body, &mut report)?;

    emit_api_client(project_root, &mut report)?;
    ensure_leptos_user_barrel_includes_api_client(project_root, &mut report)?;
    ensure_data_user_barrel_includes_generated(project_root, &mut report)?;
    ensure_leptos_user_barrel_includes_data(project_root, &mut report)?;

    sink.info(format!("{STEP_LABEL}: {} written, {} skipped", report.written.len(), report.skipped.len()));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn data_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("data").join("generated")
}

fn api_client_path(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("leptos").join("api_client.rs")
}

fn emit_resource(project_root: &Path, resource: &ResourceState, data_dir: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let table = resource.name.as_str();
    let marker = header::marker_for_resource(project_root, table)?;
    let body = render::render_resource_helpers(resource);
    let target = data_dir.join(format!("{table}.rs"));
    write_file(&target, &format!("{marker}{body}"), report)?;
    Ok(())
}

fn emit_api_client(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let target = api_client_path(project_root);
    if target.exists() {
        report.skipped.push(target);
        return Ok(());
    }
    let body = render::render_api_client_module();
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("api_client target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(&target, &body)?;
    report.written.push(target);
    Ok(())
}

fn ensure_leptos_user_barrel_includes_api_client(project_root: &Path, report: &mut EmitReport) -> BlastResult<()> {
    let user_barrel = project_root.join("src").join("transport").join("leptos").join("mod.rs");
    let existing = match fs::read_to_string(&user_barrel) {
        Ok(s) => s,
        Err(_io) => return Ok(()),
    };
    if existing.contains("pub mod api_client;") {
        return Ok(());
    }
    let updated = match existing.ends_with('\n') {
        true => format!("{existing}pub mod api_client;\n"),
        false => format!("{existing}\npub mod api_client;\n"),
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
        Err(_io) => return Ok(()),
    };
    if existing.contains("pub mod data;") {
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

fn read_existing(target: &Path) -> BlastResult<Option<String>> {
    if !target.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(target)?;
    Ok(Some(body))
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("leptos_data target has no parent: {}", target.display())))?;
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
            resource::{AuthMode, FieldState, FieldVariant, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION},
            save_app, save_resource, AppState, GenLevel, SqlType,
        },
    };

    fn make_posts_with_all_verbs(level: GenLevel) -> ResourceState {
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
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::AdminOnly,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Delete,
            VerbState {
                auth: AuthMode::AdminOnly,
                list_options: None,
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
    fn emits_per_verb_helpers_for_full_verb_set() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Composables);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = match run(root, &mut sink, &mut progress) {
            Ok(r) => r,
            Err(e) => panic!("run: {e}"),
        };

        let data_file = root.join("src/transport/leptos/data/generated/posts.rs");
        let data_barrel = root.join("src/transport/leptos/data/generated/mod.rs");
        let api_client = root.join("src/transport/leptos/api_client.rs");

        assert!(data_file.exists(), "data file must exist");
        assert!(data_barrel.exists(), "data barrel must exist");
        assert!(api_client.exists(), "api_client.rs must exist");

        let body = match fs::read_to_string(&data_file) {
            Ok(s) => s,
            Err(e) => panic!("read posts.rs: {e}"),
        };

        assert!(body.starts_with("// AUTO-GENERATED from "), "marker header expected: {body}");
        assert!(body.contains("pub async fn load_posts_list"), "missing list helper: {body}");
        assert!(body.contains("pub async fn load_posts_one"), "missing get helper: {body}");
        assert!(body.contains("pub async fn do_posts_create"), "missing create helper: {body}");
        assert!(body.contains("pub async fn do_posts_update"), "missing update helper: {body}");
        assert!(body.contains("pub async fn do_posts_delete"), "missing delete helper: {body}");

        let written: Vec<&PathBuf> = report.written.iter().collect();
        assert!(written.iter().any(|p| *p == &data_file), "data file must be reported written");
        assert!(written.iter().any(|p| *p == &api_client), "api_client must be reported written");
    }

    #[test]
    fn each_helper_has_both_cfg_branches() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Composables);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let body = match fs::read_to_string(root.join("src/transport/leptos/data/generated/posts.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read posts.rs: {e}"),
        };

        let ssr_count = body.matches("#[cfg(not(target_arch = \"wasm32\"))]").count();
        let wasm_count = body.matches("#[cfg(target_arch = \"wasm32\")]").count();
        assert!(ssr_count >= 5, "must have >= 5 ssr cfg blocks (one per verb), got {ssr_count}: {body}");
        assert!(wasm_count >= 5, "must have >= 5 wasm cfg blocks (one per verb), got {wasm_count}: {body}");
    }

    #[test]
    fn ssr_branch_calls_flow_wasm_branch_calls_api_client() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Composables);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let body = match fs::read_to_string(root.join("src/transport/leptos/data/generated/posts.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read posts.rs: {e}"),
        };

        assert!(body.contains("crate::flows::generated::posts::list::run"), "ssr list must call flow: {body}");
        assert!(body.contains("crate::flows::generated::posts::create::run"), "ssr create must call flow: {body}");
        assert!(body.contains("crate::flows::generated::posts::delete::run"), "ssr delete must call flow: {body}");
        assert!(body.contains("crate::transport::leptos::api_client::get_json"), "wasm get must call api_client: {body}");
        assert!(body.contains("crate::transport::leptos::api_client::post_json"), "wasm post must call api_client: {body}");
        assert!(body.contains("crate::transport::leptos::api_client::patch_json"), "wasm patch must call api_client: {body}");
        assert!(body.contains("crate::transport::leptos::api_client::delete"), "wasm delete must call api_client: {body}");
    }

    #[test]
    fn signatures_match_locked_contract() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Composables);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let body = match fs::read_to_string(root.join("src/transport/leptos/data/generated/posts.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read posts.rs: {e}"),
        };

        assert!(
            body.contains("pub async fn load_posts_list(query: ListQuery) -> ::std::result::Result<ListResponse<PostPublic>, MeltDown>"),
            "list signature mismatch: {body}"
        );
        assert!(
            body.contains("pub async fn load_posts_one(id: i64) -> ::std::result::Result<PostPublic, MeltDown>"),
            "get signature mismatch: {body}"
        );
        assert!(
            body.contains("pub async fn do_posts_create(input: PostInsertable) -> ::std::result::Result<PostPublic, MeltDown>"),
            "create signature mismatch: {body}"
        );
        assert!(
            body.contains("pub async fn do_posts_update(id: i64, patch: PostPatch) -> ::std::result::Result<PostPublic, MeltDown>"),
            "update signature mismatch: {body}"
        );
        assert!(body.contains("pub async fn do_posts_delete(id: i64) -> ::std::result::Result<(), MeltDown>"), "delete signature mismatch: {body}");
    }

    #[test]
    fn skips_resources_below_composables_gen_level() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Types);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let data_file = root.join("src/transport/leptos/data/generated/posts.rs");
        assert!(!data_file.exists(), "must NOT emit when gen_level < Composables");
    }

    #[test]
    fn idempotent_second_run_skips_unchanged() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Composables);
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

        assert!(!second.skipped.is_empty(), "second run must skip unchanged files");
    }

    #[test]
    fn no_resources_emits_gitkeep_and_empty_barrel() {
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

        let keep = root.join("src/transport/leptos/data/generated/.gitkeep");
        let barrel = root.join("src/transport/leptos/data/generated/mod.rs");
        assert!(keep.exists(), ".gitkeep expected when no resources");
        assert!(barrel.exists(), "empty barrel expected");
    }

    #[test]
    fn api_client_emits_isomorphic_helpers() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Composables);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let api_client = match fs::read_to_string(root.join("src/transport/leptos/api_client.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read api_client: {e}"),
        };

        assert!(api_client.contains("pub async fn get_json"), "get_json missing: {api_client}");
        assert!(api_client.contains("pub async fn post_json"), "post_json missing: {api_client}");
        assert!(api_client.contains("pub async fn patch_json"), "patch_json missing: {api_client}");
        assert!(api_client.contains("pub async fn delete"), "delete missing: {api_client}");
        assert!(api_client.contains("::gloo_net::http::Request::"), "must use gloo_net::http::Request: {api_client}");
        assert!(api_client.contains("#[cfg(target_arch = \"wasm32\")]"), "api_client body must be wasm-cfg-gated: {api_client}");
    }

    #[test]
    fn overwrites_placeholder_stub_emitted_by_leptos_pages() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let resource = make_posts_with_all_verbs(GenLevel::Pages);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut progress = NullProgress;

        match crate::codegen::leptos_pages::run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("leptos_pages run: {e}"),
        }

        let stub_body = match fs::read_to_string(root.join("src/transport/leptos/data/generated/posts.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read posts.rs after leptos_pages: {e}"),
        };
        assert!(stub_body.contains("not implemented"), "leptos_pages should emit a placeholder stub: {stub_body}");

        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("leptos_data run: {e}"),
        }

        let real_body = match fs::read_to_string(root.join("src/transport/leptos/data/generated/posts.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read posts.rs after leptos_data: {e}"),
        };
        assert!(!real_body.contains("not implemented"), "leptos_data must overwrite placeholder; still contains 'not implemented': {real_body}");
        assert!(real_body.contains("crate::flows::generated::posts::list::run"), "real body must call flow: {real_body}");
        assert!(real_body.contains("crate::transport::leptos::api_client::"), "real body must reference api_client: {real_body}");
    }

    #[test]
    fn top_barrel_lists_emitted_resources_alphabetically() {
        let tmp = match TempDir::new() {
            Ok(t) => t,
            Err(e) => panic!("tempdir: {e}"),
        };
        let root = tmp.path();
        let mut posts = make_posts_with_all_verbs(GenLevel::Composables);
        let mut users = make_posts_with_all_verbs(GenLevel::Composables);
        users.name = ResourceName::new("users");
        posts.name = ResourceName::new("posts");
        seed_project(root, &[users, posts]);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        match run(root, &mut sink, &mut progress) {
            Ok(_r) => {}
            Err(e) => panic!("run: {e}"),
        }

        let barrel = match fs::read_to_string(root.join("src/transport/leptos/data/generated/mod.rs")) {
            Ok(s) => s,
            Err(e) => panic!("read barrel: {e}"),
        };

        let posts_idx = match barrel.find("pub mod posts;") {
            Some(i) => i,
            None => panic!("posts not in barrel: {barrel}"),
        };
        let users_idx = match barrel.find("pub mod users;") {
            Some(i) => i,
            None => panic!("users not in barrel: {barrel}"),
        };
        assert!(posts_idx < users_idx, "barrel must be alphabetically sorted: {barrel}");
    }
}
