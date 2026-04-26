//! Pipeline entry point — walks every Primer state file and emits a
//! per-resource composable module under
//! `frontend/src/generated/composables/<resource>.ts`. Also writes an
//! `index.ts` barrel for the directory so consumers can `import * as
//! composables from '@/generated/composables'` if they prefer.
//!
//! Idempotent: reads existing files first and skips identical writes.
//! `EmitReport` records both written and skipped paths so `gen all` can
//! tally accurate counts.

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::composables_v2::naming::file_stem;
use crate::codegen::composables_v2::render::build_resource_ts;
use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "composables v2";

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    let resources = ir_loader::load_resource_states(project_root)?;
    let total = resources.len() as u64;
    progress.tick(0, total);

    let out_dir = composables_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    let mut report = EmitReport::default();
    let mut emitted: Vec<String> = Vec::new();
    let mut processed: u64 = 0;

    for r in &resources {
        let table = r.name.as_str();
        if r.verbs.is_empty() {
            sink.debug(format!(
                "composables v2: resource {} has no verbs; skipping",
                table
            ));
            processed += 1;
            progress.tick(processed, total);
            continue;
        }
        let marker = header::marker_for_resource(project_root, table)?;
        let body = format!("{}{}", marker, build_resource_ts(r));
        let path = out_dir.join(format!("{}.ts", file_stem(table)));
        write_file(&path, &body, &mut report)?;
        emitted.push(table.to_string());
        processed += 1;
        progress.tick(processed, total);
    }

    let app_marker = header::marker_for_app(project_root)?;
    let mut barrel = app_marker;
    let mut barrel_names = emitted.clone();
    barrel_names.sort();
    for name in &barrel_names {
        barrel.push_str(&format!("export * from './{}'\n", name));
    }
    let barrel_path = out_dir.join("index.ts");
    write_file(&barrel_path, &barrel, &mut report)?;

    sink.info(format!(
        "{}: {} written, {} skipped",
        STEP_LABEL,
        report.written.len(),
        report.skipped.len()
    ));

    Ok(report)
}

fn composables_dir(project_root: &Path) -> PathBuf {
    project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("composables")
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
            "composables v2 target has no parent: {}",
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
        Some(_changed) => {}
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
        AuthMode, FieldState, FieldVariant, FilterKind, ListOptions, PayloadShape, ResourceState,
        TopicScope, Verb, VerbState, WsEventsState, RESOURCE_SCHEMA_VERSION,
    };
    use crate::state::AppState;
    use crate::state::SqlType;
    use crate::state::{save_app, save_resource};
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_users_resource(
        verbs: Vec<Verb>,
        ws_events: Option<WsEventsState>,
    ) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let all_variants: BTreeSet<FieldVariant> = [
            FieldVariant::Db,
            FieldVariant::Insertable,
            FieldVariant::Patch,
            FieldVariant::Public,
        ]
        .into_iter()
        .collect();
        let id_variants: BTreeSet<FieldVariant> =
            [FieldVariant::Db, FieldVariant::Public].into_iter().collect();

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
            FieldName::new("email"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_variants,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verb_map: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in verbs {
            let list_opts = match v {
                Verb::List => Some(ListOptions {
                    paginated: true,
                    filterable_columns: {
                        let mut m: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
                        m.insert(FieldName::new("email"), FilterKind::IlikeContains);
                        m
                    },
                    sortable_columns: {
                        let mut s: BTreeSet<FieldName> = BTreeSet::new();
                        s.insert(FieldName::new("id"));
                        s
                    },
                    default_sort: Some(FieldName::new("id")),
                    max_page_size: Some(100),
                }),
                _other => None,
            };
            verb_map.insert(
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
            verbs: verb_map,
            ws_events,
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
    fn emits_full_surface_when_all_verbs_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(
            vec![
                Verb::List,
                Verb::Get,
                Verb::Create,
                Verb::Update,
                Verb::Delete,
            ],
            None,
        );
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("composables run");

        let path = root.join("frontend/src/generated/composables/users.ts");
        assert!(path.exists(), "users.ts should exist");
        let body = fs::read_to_string(&path).expect("read users.ts");

        assert!(body.starts_with("// AUTO-GENERATED from "), "marker required");
        assert!(body.contains("export function useUsersList"));
        assert!(body.contains("export function useUser("));
        assert!(body.contains("export function useCreateUser"));
        assert!(body.contains("export function useUpdateUser"));
        assert!(body.contains("export function useDeleteUser"));

        let barrel =
            fs::read_to_string(root.join("frontend/src/generated/composables/index.ts"))
                .expect("read barrel");
        assert!(barrel.contains("export * from './users'"));

        assert!(report.written.iter().any(|p| p == &path));
    }

    #[test]
    fn omits_disabled_verbs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(vec![Verb::List, Verb::Get], None);
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ = run(root, &mut sink, &mut progress).expect("composables run");

        let body = fs::read_to_string(
            root.join("frontend/src/generated/composables/users.ts"),
        )
        .expect("read users.ts");

        assert!(body.contains("useUsersList"), "list emitted");
        assert!(body.contains("useUser("), "single emitted");
        assert!(!body.contains("useCreateUser"), "create not emitted");
        assert!(!body.contains("useUpdateUser"), "update not emitted");
        assert!(!body.contains("useDeleteUser"), "delete not emitted");
    }

    #[test]
    fn list_emits_all_three_tier_triggers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(vec![Verb::List], None);
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ = run(root, &mut sink, &mut progress).expect("composables run");

        let body = fs::read_to_string(
            root.join("frontend/src/generated/composables/users.ts"),
        )
        .expect("read users.ts");

        // Tier 1 (static): refetch fn + watchEffect on URL state.
        assert!(body.contains("const refetch"), "refetch fn present");
        assert!(body.contains("watchEffect"), "URL-driven refetch");
        // Tier 2 (poll): setInterval guarded by visibilityState.
        assert!(body.contains("opts.poll !== undefined"), "poll guard");
        assert!(
            body.contains("document.visibilityState === 'visible'"),
            "visibility guard for poll"
        );
        assert!(body.contains("clearInterval(pollTimer)"), "poll cleanup");
        // Tier 3 (live): useChannel + watch on lastEvent.
        assert!(body.contains("opts.live === true"), "live guard");
        assert!(body.contains("useChannel('users/all')"), "live channel topic");
        assert!(body.contains("channel.lastEvent"), "channel watch");
    }

    #[test]
    fn mutations_emit_bus_events_on_success() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(
            vec![Verb::Create, Verb::Update, Verb::Delete],
            None,
        );
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ = run(root, &mut sink, &mut progress).expect("composables run");

        let body = fs::read_to_string(
            root.join("frontend/src/generated/composables/users.ts"),
        )
        .expect("read users.ts");

        assert!(
            body.contains("emit('users:created', result.data)"),
            "create emits bus event"
        );
        assert!(
            body.contains("emit('users:updated', result.data)"),
            "update emits bus event"
        );
        assert!(
            body.contains("emit('users:deleted', { id })"),
            "delete emits bus event with id"
        );
    }

    #[test]
    fn abort_signal_plumbed_through_fetches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(vec![Verb::List, Verb::Get], None);
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ = run(root, &mut sink, &mut progress).expect("composables run");

        let body = fs::read_to_string(
            root.join("frontend/src/generated/composables/users.ts"),
        )
        .expect("read users.ts");

        assert!(body.contains("getNavAbortSignal()"), "nav signal pulled");
        assert!(body.contains("AbortController"), "controller created");
        assert!(body.contains("controller.signal"), "signal forwarded to API");
    }

    #[test]
    fn forbidden_patterns_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(
            vec![
                Verb::List,
                Verb::Get,
                Verb::Create,
                Verb::Update,
                Verb::Delete,
            ],
            None,
        );
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ = run(root, &mut sink, &mut progress).expect("composables run");

        let body = fs::read_to_string(
            root.join("frontend/src/generated/composables/users.ts"),
        )
        .expect("read users.ts");

        assert!(!body.contains(": any"), "no `: any`");
        assert!(!body.contains(" as any"), "no `as any`");
        assert!(!body.contains("console.log"), "no console.log");
        assert!(!body.contains("new WebSocket"), "no raw WebSocket");
        // Direct `fetch(` should not appear; we go through the API client.
        // (`refetch` is fine — that's our own identifier.)
        assert!(
            !body.contains(" fetch(") && !body.contains("\tfetch(") && !body.contains("=fetch("),
            "no raw fetch call"
        );
        // No literal `||` or `??` fallbacks against literals.
        assert!(!body.contains("|| []"), "no || [] fallback");
        assert!(!body.contains("|| {}"), "no || {{}} fallback");
        assert!(!body.contains("?? []"), "no ?? [] fallback");
        assert!(!body.contains("?? {}"), "no ?? {{}} fallback");
        assert!(!body.contains("?? 0"), "no ?? 0 fallback");
        assert!(!body.contains("?? false"), "no ?? false fallback");
    }

    #[test]
    fn snake_case_types_imported_intact() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(
            vec![Verb::List, Verb::Get, Verb::Create, Verb::Update],
            None,
        );
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ = run(root, &mut sink, &mut progress).expect("composables run");

        let body = fs::read_to_string(
            root.join("frontend/src/generated/composables/users.ts"),
        )
        .expect("read users.ts");

        // Type imports use the canonical snake_case-friendly Pascal names
        // (`UserPublic`, `UserInsertable`, `UserPatch`, `UserFilter`)
        // which themselves carry snake_case fields by structs codegen.
        assert!(body.contains("UserPublic"), "UserPublic referenced");
        assert!(body.contains("UserInsertable"), "UserInsertable referenced");
        assert!(body.contains("UserPatch"), "UserPatch referenced");
        assert!(body.contains("UserFilter"), "UserFilter referenced");
        // Page-size opt key must remain camelCase (FE framework surface).
        assert!(body.contains("page_size: url.pageSize.value"), "wire camel→snake bridge");
    }

    #[test]
    fn live_per_row_topic_emitted_when_ws_per_row() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource = synth_users_resource(
            vec![Verb::Get],
            Some(WsEventsState {
                trigger_columns: BTreeSet::new(),
                payload_shape: PayloadShape::Public,
                topic_scope: TopicScope::PerRow,
            }),
        );
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _ = run(root, &mut sink, &mut progress).expect("composables run");

        let body = fs::read_to_string(
            root.join("frontend/src/generated/composables/users.ts"),
        )
        .expect("read users.ts");

        assert!(
            body.contains("useChannel(`users/${idForTopic.value}`)"),
            "per-row WS topic interpolation"
        );
    }

    #[test]
    fn idempotent_second_run_skips_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let resource =
            synth_users_resource(vec![Verb::List, Verb::Create, Verb::Update], None);
        write_synth_project(root, resource);

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let _first = run(root, &mut sink, &mut progress).expect("first run");
        let second = run(root, &mut sink, &mut progress).expect("second run");

        assert!(
            second.written.is_empty(),
            "second run wrote {:?}",
            second.written
        );
        assert!(!second.skipped.is_empty(), "second run skipped nothing");
    }
}
