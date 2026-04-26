use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::codegen::vue::barrels::{build_resource_index_ts, build_root_barrel_ts};
use crate::codegen::vue::form::build_form_sfc;
use crate::codegen::vue::list::build_list_sfc;
use crate::codegen::vue::marker::vue_marker_for_resource;
use crate::codegen::vue::plan::ResourcePlan;
use crate::codegen::vue::report::EmitReport;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};

const STEP_LABEL: &str = "vue components";

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    let resources = ir_loader::load_resource_states(project_root)?;
    let total = resources.len() as u64;
    progress.tick(0, total);

    let mut report = EmitReport::default();
    let mut emitted_resource_dirs: Vec<String> = Vec::new();
    let mut processed: u64 = 0;

    for r in &resources {
        let plan = ResourcePlan::from(r);
        if !plan.has_any() {
            sink.debug(format!(
                "vue: resource {} has no create/update/list verbs; skipping",
                r.name.as_str()
            ));
            processed += 1;
            progress.tick(processed, total);
            continue;
        }

        let resource_dir = components_dir(project_root).join(r.name.as_str());
        fs::create_dir_all(&resource_dir)?;
        let html_marker = vue_marker_for_resource(project_root, r.name.as_str())?;
        let ts_marker = header::marker_for_resource(project_root, r.name.as_str())?;

        if plan.emit_form {
            let body = format!("{}{}", html_marker, build_form_sfc(r));
            let path = resource_dir.join("Form.vue");
            write_file(&path, &body, &mut report)?;
        }
        if plan.emit_list {
            let body = format!("{}{}", html_marker, build_list_sfc(r));
            let path = resource_dir.join("List.vue");
            write_file(&path, &body, &mut report)?;
        }

        let index_body = format!("{}{}", ts_marker, build_resource_index_ts(&plan));
        let index_path = resource_dir.join("index.ts");
        write_file(&index_path, &index_body, &mut report)?;

        emitted_resource_dirs.push(r.name.as_str().to_string());
        processed += 1;
        progress.tick(processed, total);
    }

    let app_marker = header::marker_for_app(project_root)?;
    let barrel_body = format!(
        "{}{}",
        app_marker,
        build_root_barrel_ts(&emitted_resource_dirs)
    );
    let root_dir = components_dir(project_root);
    fs::create_dir_all(&root_dir)?;
    let barrel_path = root_dir.join("index.ts");
    write_file(&barrel_path, &barrel_body, &mut report)?;

    sink.info(format!(
        "{}: {} written, {} skipped",
        STEP_LABEL,
        report.written.len(),
        report.skipped.len()
    ));

    Ok(report)
}

fn components_dir(project_root: &Path) -> PathBuf {
    project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("components")
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
        BlastError::Invalid(format!("vue target has no parent: {}", target.display()))
    })?;
    fs::create_dir_all(parent)?;
    let existing = read_existing(target)?;
    match existing {
        Some(prev) if prev == body => {
            report.skipped.push(target.to_path_buf());
            return Ok(());
        }
        Some(_unchanged_or_changed) => {}
        None => {}
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
        AuthMode, FieldState, FieldVariant, FilterKind, ListOptions, ResourceState, Verb,
        VerbState, RESOURCE_SCHEMA_VERSION,
    };
    use crate::state::AppState;
    use crate::state::SqlType;
    use crate::state::{save_app, save_resource};
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_widget_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let mut all_variants = BTreeSet::new();
        all_variants.insert(FieldVariant::Db);
        all_variants.insert(FieldVariant::Insertable);
        all_variants.insert(FieldVariant::Patch);
        all_variants.insert(FieldVariant::Public);

        let mut id_variants = BTreeSet::new();
        id_variants.insert(FieldVariant::Db);
        id_variants.insert(FieldVariant::Public);

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: id_variants,
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("name"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_variants.clone(),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("active"),
            FieldState {
                sql_type: SqlType::new("Bool"),
                variants: all_variants.clone(),
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("created_at"),
            FieldState {
                sql_type: SqlType::new("Timestamptz"),
                variants: {
                    let mut s = BTreeSet::new();
                    s.insert(FieldVariant::Db);
                    s.insert(FieldVariant::Public);
                    s
                },
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        let mut sortable: BTreeSet<FieldName> = BTreeSet::new();
        sortable.insert(FieldName::new("created_at"));
        let mut filterable: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
        filterable.insert(FieldName::new("name"), FilterKind::Eq);
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: filterable,
                    sortable_columns: sortable,
                    default_sort: Some(FieldName::new("created_at")),
                    max_page_size: Some(100),
                }),
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("widgets"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
        }
    }

    fn write_synth_project(root: &Path) {
        let state_dir = root.join("storage").join("blast").join("state");
        fs::create_dir_all(state_dir.join("resources")).expect("mk state dir");
        let mut app = AppState::default();
        app.canonicalize();
        save_app(&state_dir, &app).expect("save app");
        let res = synth_widget_resource();
        save_resource(&state_dir, &res).expect("save resource");
    }

    #[test]
    fn emits_form_and_list_with_headers_and_primevue_tags() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("vue codegen runs");

        let form_path = root.join("frontend/src/generated/components/widgets/Form.vue");
        let list_path = root.join("frontend/src/generated/components/widgets/List.vue");
        let res_index = root.join("frontend/src/generated/components/widgets/index.ts");
        let root_index = root.join("frontend/src/generated/components/index.ts");

        assert!(form_path.exists(), "Form.vue should exist");
        assert!(list_path.exists(), "List.vue should exist");
        assert!(res_index.exists(), "resource index.ts should exist");
        assert!(root_index.exists(), "root index.ts should exist");

        let form_body = fs::read_to_string(&form_path).expect("read form");
        let list_body = fs::read_to_string(&list_path).expect("read list");

        assert!(
            form_body.starts_with("<!-- AUTO-GENERATED from "),
            "Form.vue must start with HTML marker; got: {}",
            &form_body[..form_body.len().min(80)]
        );
        assert!(
            list_body.starts_with("<!-- AUTO-GENERATED from "),
            "List.vue must start with HTML marker"
        );

        assert!(form_body.contains("<InputText"), "Form should use InputText");
        assert!(form_body.contains("<Checkbox"), "Form should use Checkbox for bool");
        assert!(form_body.contains("<Button"), "Form should use Button");
        assert!(list_body.contains("<DataTable"), "List should use DataTable");
        assert!(list_body.contains("<Column"), "List should use Column");

        assert!(
            form_body.contains("@/generated/validators/widgets"),
            "Form should import validator"
        );
        assert!(
            list_body.contains("@/generated/queries/widgets_list"),
            "List should import per-resource query helpers"
        );
        assert!(
            form_body.contains("WidgetInsertable"),
            "Form should emit Widget* payload type"
        );

        // Polish: Form.vue surfaces loading/error/feedback affordances.
        assert!(
            form_body.contains("import Message from 'primevue/message'"),
            "Form should import Message for inline error banner"
        );
        assert!(
            form_body.contains("from 'primevue/usetoast'"),
            "Form should pull useToast for validation feedback"
        );
        assert!(
            form_body.contains(":loading=\"submitting\""),
            "Form submit button should bind loading from props"
        );
        assert!(
            form_body.contains(":disabled=\"submitting\""),
            "Form submit button should disable while submitting"
        );
        assert!(
            form_body.contains("submitError"),
            "Form should surface a submit error prop"
        );
        assert!(
            form_body.contains("retry: []"),
            "Form should declare a retry emit"
        );
        assert!(
            form_body.contains("@click=\"onRetry\""),
            "Form retry button should be wired"
        );
        assert!(
            form_body.contains(":aria-invalid="),
            "Form fields should expose aria-invalid for screen readers"
        );
        assert!(
            form_body.contains("class=\"field-error p-error\""),
            "Form should style field errors via PrimeVue's p-error class"
        );
        assert!(
            form_body.contains("role=\"alert\""),
            "Form field errors should use role=alert"
        );

        // Polish: List.vue carries loading + empty states + a11y hints.
        assert!(
            list_body.contains("import ProgressSpinner from 'primevue/progressspinner'"),
            "List should import ProgressSpinner for loading state"
        );
        assert!(
            list_body.contains("<template #loading>"),
            "List should render a loading slot"
        );
        assert!(
            list_body.contains("<template #empty>"),
            "List should render an empty-state slot"
        );
        assert!(
            list_body.contains("<ProgressSpinner"),
            "List loading slot should render the spinner"
        );
        assert!(
            list_body.contains("No widgets to show yet."),
            "List empty state should mention the resource"
        );
        assert!(
            list_body.contains("'aria-label': 'widgets list'"),
            "List should label the table for a11y"
        );

        assert!(
            report.written.iter().any(|p| p == &form_path),
            "Form should be in written list"
        );
        assert!(
            report.written.iter().any(|p| p == &list_path),
            "List should be in written list"
        );
    }

    fn synth_widget_resource_with_delete() -> ResourceState {
        let mut r = synth_widget_resource();
        r.verbs.insert(
            Verb::Delete,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );
        r.verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );
        r
    }

    fn write_synth_project_with_delete(root: &Path) {
        let state_dir = root.join("storage").join("blast").join("state");
        fs::create_dir_all(state_dir.join("resources")).expect("mk state dir");
        let mut app = AppState::default();
        app.canonicalize();
        save_app(&state_dir, &app).expect("save app");
        let res = synth_widget_resource_with_delete();
        save_resource(&state_dir, &res).expect("save resource");
    }

    #[test]
    fn list_renders_actions_and_confirm_dialog_when_delete_verb_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project_with_delete(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _report = run(root, &mut sink, &mut progress).expect("vue codegen runs");

        let list_path = root.join("frontend/src/generated/components/widgets/List.vue");
        let list_body = fs::read_to_string(&list_path).expect("read list");

        assert!(
            list_body.contains("import ConfirmDialog from 'primevue/confirmdialog'"),
            "List with Delete verb should import ConfirmDialog"
        );
        assert!(
            list_body.contains("from 'primevue/useconfirm'"),
            "List with Delete verb should pull useConfirm"
        );
        assert!(
            list_body.contains("from 'primevue/usetoast'"),
            "List with Delete verb should pull useToast for feedback"
        );
        assert!(
            list_body.contains("<ConfirmDialog />"),
            "List should mount the ConfirmDialog component"
        );
        assert!(
            list_body.contains("header: 'Confirm delete'"),
            "List delete handler should configure confirm header"
        );
        assert!(
            list_body.contains("delete: [row: WidgetPublic]"),
            "List should declare a typed delete emit"
        );
        assert!(
            list_body.contains("edit: [row: WidgetPublic]"),
            "List should declare a typed edit emit when Update verb present"
        );
        assert!(
            list_body.contains("aria-label=\"Delete row\""),
            "List delete button should be labelled for a11y"
        );
        assert!(
            list_body.contains("aria-label=\"Edit row\""),
            "List edit button should be labelled for a11y"
        );
        assert!(
            list_body.contains("header=\"Actions\""),
            "List should render an Actions column when mutating verbs exist"
        );
    }

    #[test]
    fn idempotent_second_run_skips_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_synth_project(root);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ignored = run(root, &mut sink, &mut progress).expect("first run");
        let second = run(root, &mut sink, &mut progress).expect("second run");

        assert!(
            second.written.is_empty(),
            "second run should write nothing; wrote {:?}",
            second.written
        );
        assert!(!second.skipped.is_empty(), "second run should skip files");
    }
}
