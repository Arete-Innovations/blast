//! Per-resource CRUD page emitter.
//!
//! For each Primer resource with relevant verbs enabled, emits Vue page SFCs
//! under `frontend/src/pages/<resource>/`. Pages wrap `<PageShell layout="...">`,
//! consume generated components and forward-reference lane-5 composables.
//!
//! Verb → page mapping (per SPEC_FRONTEND_ROUTING):
//!   List   → <Resource>ListPage.vue   (layout: table)
//!   Get    → <Resource>DetailPage.vue (layout: cards)
//!   Create → <Resource>CreatePage.vue (layout: cards)
//!   Update → <Resource>EditPage.vue   (layout: cards)
//!   Delete → not a page; action lives in detail/list

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::ir_loader;
use crate::codegen::pages::emit::{
    build_create_page, build_detail_page, build_edit_page, build_list_page,
};
use crate::codegen::vue::marker::vue_marker_for_resource;
use crate::codegen::vue::naming::{pascal_case, singularize};
use crate::codegen::vue::report::EmitReport;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state::resource::Verb;

const STEP_LABEL: &str = "crud pages";

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    let resources = ir_loader::load_resource_states(project_root)?;
    let total = resources.len() as u64;
    progress.tick(0, total);

    let mut report = EmitReport::default();
    let mut processed: u64 = 0;

    for r in &resources {
        let table = r.name.as_str();
        let singular = match r.singular_override.as_deref() {
            Some(s) => s.to_string(),
            None => singularize(table), // allow: singular_override is a naming hint, absence is not a failure
        };
        let pascal = pascal_case(&singular);

        let has_list = r.verbs.contains_key(&Verb::List);
        let has_get = r.verbs.contains_key(&Verb::Get);
        let has_create = r.verbs.contains_key(&Verb::Create);
        let has_update = r.verbs.contains_key(&Verb::Update);

        if !has_list && !has_get && !has_create && !has_update {
            sink.debug(format!(
                "pages: resource {} has no list/get/create/update verbs; skipping",
                table
            ));
            processed += 1;
            progress.tick(processed, total);
            continue;
        }

        let resource_dir = pages_dir(project_root).join(table);
        fs::create_dir_all(&resource_dir)?;

        let html_marker = vue_marker_for_resource(project_root, table)?;

        if has_list {
            let body = format!("{}{}", html_marker, build_list_page(r));
            let path = resource_dir.join(format!("{}ListPage.vue", pascal));
            write_file(&path, &body, &mut report)?;
        }
        if has_get {
            let body = format!("{}{}", html_marker, build_detail_page(r));
            let path = resource_dir.join(format!("{}DetailPage.vue", pascal));
            write_file(&path, &body, &mut report)?;
        }
        if has_create {
            let body = format!("{}{}", html_marker, build_create_page(r));
            let path = resource_dir.join(format!("{}CreatePage.vue", pascal));
            write_file(&path, &body, &mut report)?;
        }
        if has_update {
            let body = format!("{}{}", html_marker, build_edit_page(r));
            let path = resource_dir.join(format!("{}EditPage.vue", pascal));
            write_file(&path, &body, &mut report)?;
        }

        processed += 1;
        progress.tick(processed, total);
    }

    sink.info(format!(
        "{}: {} written, {} skipped",
        STEP_LABEL,
        report.written.len(),
        report.skipped.len()
    ));

    Ok(report)
}

fn pages_dir(project_root: &Path) -> PathBuf {
    project_root
        .join("frontend")
        .join("src")
        .join("pages")
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
        BlastError::Invalid(format!("pages target has no parent: {}", target.display()))
    })?;
    fs::create_dir_all(parent)?;
    let existing = read_existing(target)?;
    match existing {
        Some(prev) if prev == body => {
            report.skipped.push(target.to_path_buf());
            return Ok(());
        }
        Some(_changed) => {} // allow: file exists but content differs; fall through to overwrite
        None => {} // allow: file does not exist yet; fall through to write
    }
    fs::write(target, body)?;
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
    use crate::state::AppState;
    use crate::state::SqlType;
    use crate::state::{save_app, save_resource};
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_full_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let mut all_v = BTreeSet::new();
        all_v.insert(FieldVariant::Db);
        all_v.insert(FieldVariant::Insertable);
        all_v.insert(FieldVariant::Patch);
        all_v.insert(FieldVariant::Public);

        let mut id_v = BTreeSet::new();
        id_v.insert(FieldVariant::Db);
        id_v.insert(FieldVariant::Public);

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
                sql_type: SqlType::new("Varchar"),
                variants: all_v.clone(),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        let mut sortable: BTreeSet<FieldName> = BTreeSet::new();
        sortable.insert(FieldName::new("id"));
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: BTreeMap::new(),
                    sortable_columns: sortable,
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
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Delete,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("articles"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
        }
    }

    fn write_synth_project(root: &Path, resource: ResourceState) {
        let state_dir = root.join("storage").join("blast").join("state");
        fs::create_dir_all(state_dir.join("resources")).expect("mk state dir");
        let mut app = AppState::default();
        app.canonicalize();
        save_app(&state_dir, &app).expect("save app");
        save_resource(&state_dir, &resource).expect("save resource");
    }

    #[test]
    fn emits_four_pages_for_full_verb_resource() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root, synth_full_resource());

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("pages run");

        let base = root.join("frontend/src/pages/articles");
        assert!(base.join("ArticleListPage.vue").exists(), "ListPage should exist");
        assert!(base.join("ArticleDetailPage.vue").exists(), "DetailPage should exist");
        assert!(base.join("ArticleCreatePage.vue").exists(), "CreatePage should exist");
        assert!(base.join("ArticleEditPage.vue").exists(), "EditPage should exist");

        assert_eq!(report.written.len(), 4, "should write 4 pages");
        assert_eq!(report.skipped.len(), 0);
    }

    #[test]
    fn emitted_pages_have_html_marker_header() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root, synth_full_resource());

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run");

        let base = root.join("frontend/src/pages/articles");
        for name in ["ArticleListPage.vue", "ArticleDetailPage.vue", "ArticleCreatePage.vue", "ArticleEditPage.vue"] {
            let body = fs::read_to_string(base.join(name)).expect("read page");
            assert!(
                body.starts_with("<!-- AUTO-GENERATED from "),
                "{} must start with HTML marker",
                name
            );
        }
    }

    #[test]
    fn list_page_uses_table_layout() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root, synth_full_resource());

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run");

        let body = fs::read_to_string(root.join("frontend/src/pages/articles/ArticleListPage.vue"))
            .expect("read list page");
        assert!(body.contains("<PageShell layout=\"table\">"), "list page must use table layout");
    }

    #[test]
    fn detail_create_edit_pages_use_cards_layout() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root, synth_full_resource());

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run");

        let base = root.join("frontend/src/pages/articles");
        for name in ["ArticleDetailPage.vue", "ArticleCreatePage.vue", "ArticleEditPage.vue"] {
            let body = fs::read_to_string(base.join(name)).expect("read page");
            assert!(
                body.contains("<PageShell layout=\"cards\">"),
                "{} must use cards layout",
                name
            );
        }
    }

    #[test]
    fn partial_verbs_emit_only_matching_pages() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        // Resource with only List + Create verbs
        let mut r = synth_full_resource();
        r.verbs.shift_remove(&Verb::Get);
        r.verbs.shift_remove(&Verb::Update);
        r.verbs.shift_remove(&Verb::Delete);
        write_synth_project(root, r);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run");

        let base = root.join("frontend/src/pages/articles");
        assert!(base.join("ArticleListPage.vue").exists(), "ListPage should exist");
        assert!(base.join("ArticleCreatePage.vue").exists(), "CreatePage should exist");
        assert!(!base.join("ArticleDetailPage.vue").exists(), "DetailPage must not exist");
        assert!(!base.join("ArticleEditPage.vue").exists(), "EditPage must not exist");
        assert_eq!(report.written.len(), 2);
    }

    #[test]
    fn no_verbs_skips_resource() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        let mut r = synth_full_resource();
        r.verbs.clear();
        write_synth_project(root, r);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("run");

        assert_eq!(report.written.len(), 0, "no verbs → no pages");
    }

    #[test]
    fn idempotent_second_run_skips_all() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root, synth_full_resource());

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("first run");
        let second = run(root, &mut sink, &mut progress).expect("second run");

        assert!(second.written.is_empty(), "second run must write nothing");
        assert!(!second.skipped.is_empty(), "second run must skip pages");
    }

    #[test]
    fn pages_reference_generated_composables_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root, synth_full_resource());

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("run");

        let list_body = fs::read_to_string(
            root.join("frontend/src/pages/articles/ArticleListPage.vue"),
        )
        .expect("read");
        assert!(
            list_body.contains("@/generated/composables/articles"),
            "list page must import from generated composables"
        );
        assert!(
            list_body.contains("@/generated/components/articles"),
            "list page must import from generated components"
        );
        assert!(
            list_body.contains("@/generated/router/route-names"),
            "list page must import route names"
        );
    }
}
