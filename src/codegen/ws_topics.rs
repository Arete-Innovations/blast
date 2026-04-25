//! `blast gen ws` — emit Relay topic publishers per WS-enabled resource.
//!
//! For every resource state file with `ws_events = Some(...)`, write
//! `src/transport/ws/generated/<resource>.rs` containing:
//!   - `TOPIC` constant (string with `{field}` placeholders for scoped topics)
//!   - typed `Event` enum (`Created`/`Updated`/`Deleted`)
//!   - `publish_created` / `publish_updated` / `publish_deleted` helpers
//!
//! And maintain a barrel `src/transport/ws/generated/mod.rs` that re-exports
//! every per-resource module. Resources without `ws_events` are skipped.
//!
//! Topic format (matches `SPEC_RELAY.md` topic grammar `{resource}:{kind}:{value}`
//! collapsed to a path-style notation Catablast prefers):
//!   - `Global`               -> `"<resource>/all"`
//!   - `PerRow`               -> `"<resource>/by-id/{id}"`
//!   - `ScopedTo("<field>")`  -> `"<resource>/by-<field>/{<field>}"`
//!
//! Scoped publishers take the scope value as an argument and substitute it
//! into the placeholder at runtime; `Global` publishers omit the argument.
//!
//! Payload type per `WsEventsState.payload_shape`:
//!   - `Public`  -> `crate::structs::generated::<table>::<Pascal>Public`
//!   - `Admin`   -> `crate::structs::generated::<table>::<Pascal>Admin`
//!   - `IdOnly`  -> just `{ id: i64 }` inlined into the enum variants

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state::{PayloadShape, ResourceState, TopicScope, WsEventsState};

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "ws topics generation";

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);
    let mut report = EmitReport::default();

    let resources = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{STEP_LABEL}: {reason}"));
            return Err(err);
        }
    };

    let ws_resources: Vec<&ResourceState> = resources
        .iter()
        .filter(|r| r.ws_events.is_some())
        .collect();

    if ws_resources.is_empty() {
        sink.info(format!(
            "{STEP_LABEL}: no resources declare ws_events; nothing to emit"
        ));
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let generated_dir = project_root
        .join("src")
        .join("transport")
        .join("ws")
        .join("generated");
    fs::create_dir_all(&generated_dir)?;

    for resource in &ws_resources {
        let events = match &resource.ws_events {
            Some(e) => e,
            None => continue,
        };
        let table = resource.name.as_str();
        let resource_marker = header::marker_for_resource(project_root, table)?;
        let body = render_resource_module(table, events)?;
        let target = generated_dir.join(format!("{table}.rs"));
        let full = format!("{resource_marker}{body}");
        write_file(&target, &full, &mut report)?;
        sink.info(format!("emitted {}", target.display()));
    }

    let app_marker = header::marker_for_app(project_root)?;
    let mod_target = generated_dir.join("mod.rs");
    let mod_body = render_mod_rs(&ws_resources);
    let mod_full = format!("{app_marker}{mod_body}");
    write_file(&mod_target, &mod_full, &mut report)?;
    sink.info(format!("emitted {}", mod_target.display()));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn read_optional(target: &Path) -> BlastResult<Option<String>> {
    match fs::read_to_string(target) {
        Ok(s) => Ok(Some(s)),
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            _other => Err(BlastError::Io(err)),
        },
    }
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| {
        BlastError::Invalid(format!(
            "ws topics target has no parent: {}",
            target.display()
        ))
    })?;
    fs::create_dir_all(parent)?;

    let existing = read_optional(target)?;
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

fn render_mod_rs(resources: &[&ResourceState]) -> String {
    let mut out = String::new();
    out.push_str("// per-resource WS topic publishers — barrel re-export.\n\n");
    let mut names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    names.sort();
    for name in names {
        out.push_str(&format!("pub mod {name};\n"));
    }
    out
}

fn render_resource_module(table: &str, events: &WsEventsState) -> BlastResult<String> {
    let pascal = pascal_case(&singularize(table));
    let payload_ident = payload_type_ident(&pascal, events.payload_shape);

    let mut out = String::new();
    out.push_str("use catalyst::meltdown::MeltDown;\n");
    out.push_str("use catalyst::relay::publisher;\n");
    out.push_str("use catalyst::relay::registry::Registry;\n");
    out.push_str("use serde::Serialize;\n");
    out.push_str("use std::sync::Arc;\n\n");

    match payload_import_line(table, events.payload_shape) {
        Some(import) => {
            out.push_str(&import);
            out.push('\n');
        }
        None => {}
    }

    let topic_template = topic_template(table, &events.topic_scope);
    out.push_str(&format!(
        "/// Topic template. Scope placeholders (in `{{braces}}`) are filled by\n\
         /// the publisher fns below; do not format this constant by hand.\n\
         pub const TOPIC: &str = \"{topic_template}\";\n\n"
    ));

    out.push_str(&render_event_enum(events.payload_shape, &payload_ident));
    out.push('\n');

    out.push_str(&render_publishers(
        &events.topic_scope,
        events.payload_shape,
        &payload_ident,
    ));

    Ok(out)
}

fn payload_type_ident(pascal_singular: &str, shape: PayloadShape) -> String {
    match shape {
        PayloadShape::Public => format!("{pascal_singular}Public"),
        PayloadShape::Admin => format!("{pascal_singular}Admin"),
        PayloadShape::IdOnly => "_unused_".to_string(),
    }
}

fn payload_import_line(table: &str, shape: PayloadShape) -> Option<String> {
    match shape {
        PayloadShape::Public => Some(format!(
            "use crate::structs::generated::{table}::{pascal}Public;",
            pascal = pascal_case(&singularize(table))
        )),
        PayloadShape::Admin => Some(format!(
            "use crate::structs::generated::{table}::{pascal}Admin;",
            pascal = pascal_case(&singularize(table))
        )),
        PayloadShape::IdOnly => None,
    }
}

fn topic_template(table: &str, scope: &TopicScope) -> String {
    match scope {
        TopicScope::Global => format!("{table}/all"),
        TopicScope::PerRow => format!("{table}/by-id/{{id}}"),
        TopicScope::ScopedTo(field) => {
            let f = field.as_str();
            format!("{table}/by-{f}/{{{f}}}")
        }
    }
}

fn render_event_enum(shape: PayloadShape, payload_ident: &str) -> String {
    match shape {
        PayloadShape::Public | PayloadShape::Admin => format!(
            "#[derive(Debug, Clone, Serialize)]\n\
             #[serde(tag = \"type\")]\n\
             pub enum Event {{\n\
             \x20\x20\x20\x20Created({payload_ident}),\n\
             \x20\x20\x20\x20Updated({payload_ident}),\n\
             \x20\x20\x20\x20Deleted {{ id: i64 }},\n\
             }}\n"
        ),
        PayloadShape::IdOnly => String::from(
            "#[derive(Debug, Clone, Serialize)]\n\
             #[serde(tag = \"type\")]\n\
             pub enum Event {\n\
             \x20\x20\x20\x20Created { id: i64 },\n\
             \x20\x20\x20\x20Updated { id: i64 },\n\
             \x20\x20\x20\x20Deleted { id: i64 },\n\
             }\n",
        ),
    }
}

fn render_publishers(
    scope: &TopicScope,
    shape: PayloadShape,
    payload_ident: &str,
) -> String {
    let scope_args = scope_publisher_args(scope);
    let topic_expr = topic_expr_for(scope);

    let payload_arg = match shape {
        PayloadShape::Public | PayloadShape::Admin => {
            format!("payload: {payload_ident}")
        }
        PayloadShape::IdOnly => "id: i64".to_string(),
    };

    let payload_event = match shape {
        PayloadShape::Public | PayloadShape::Admin => {
            ("Event::Created(payload)".to_string(), "Event::Updated(payload)".to_string())
        }
        PayloadShape::IdOnly => (
            "Event::Created { id }".to_string(),
            "Event::Updated { id }".to_string(),
        ),
    };

    let combine_args = |extra: &str| -> String {
        if scope_args.is_empty() {
            extra.to_string()
        } else if extra.is_empty() {
            scope_args.clone()
        } else {
            format!("{scope_args}, {extra}")
        }
    };

    let mut out = String::new();
    out.push_str(&format!(
        "pub fn publish_created(\n\
         \x20\x20\x20\x20registry: &Arc<Registry>,\n\
         \x20\x20\x20\x20{args}\n\
         ) -> Result<usize, MeltDown> {{\n\
         \x20\x20\x20\x20let topic = {topic_expr};\n\
         \x20\x20\x20\x20Ok(publisher::publish(registry, &topic, &{event}))\n\
         }}\n\n",
        args = combine_args(&payload_arg),
        topic_expr = topic_expr,
        event = payload_event.0,
    ));

    out.push_str(&format!(
        "pub fn publish_updated(\n\
         \x20\x20\x20\x20registry: &Arc<Registry>,\n\
         \x20\x20\x20\x20{args}\n\
         ) -> Result<usize, MeltDown> {{\n\
         \x20\x20\x20\x20let topic = {topic_expr};\n\
         \x20\x20\x20\x20Ok(publisher::publish(registry, &topic, &{event}))\n\
         }}\n\n",
        args = combine_args(&payload_arg),
        topic_expr = topic_expr,
        event = payload_event.1,
    ));

    out.push_str(&format!(
        "pub fn publish_deleted(\n\
         \x20\x20\x20\x20registry: &Arc<Registry>,\n\
         \x20\x20\x20\x20{args}\n\
         ) -> Result<usize, MeltDown> {{\n\
         \x20\x20\x20\x20let topic = {topic_expr};\n\
         \x20\x20\x20\x20Ok(publisher::publish(registry, &topic, &Event::Deleted {{ id }}))\n\
         }}\n",
        args = combine_args("id: i64"),
        topic_expr = topic_expr,
    ));

    out
}

fn scope_publisher_args(scope: &TopicScope) -> String {
    match scope {
        TopicScope::Global => String::new(),
        TopicScope::PerRow => String::new(),
        TopicScope::ScopedTo(field) => format!("{}: i64", field.as_str()),
    }
}

fn topic_expr_for(scope: &TopicScope) -> String {
    match scope {
        TopicScope::Global => "TOPIC.to_string()".to_string(),
        TopicScope::PerRow => "TOPIC.replace(\"{id}\", &id.to_string())".to_string(),
        TopicScope::ScopedTo(field) => {
            let f = field.as_str();
            format!("TOPIC.replace(\"{{{f}}}\", &{f}.to_string())")
        }
    }
}

fn singularize(table: &str) -> String {
    for suffix in ["sses", "shes", "ches", "xes", "zes"] {
        match table.strip_suffix(suffix) {
            Some(stem) => return format!("{}{}", stem, &suffix[..suffix.len() - 2]),
            None => continue,
        }
    }
    match table.strip_suffix("ies") {
        Some(stem) => format!("{stem}y"),
        None => match table.strip_suffix('s') {
            Some(stem) => stem.to_string(),
            None => table.to_string(),
        },
    }
}

fn pascal_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut upper_next = true;
    for ch in input.chars() {
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::{NullProgress, NullSink};
    use crate::state::names::{AuthScopeField, FieldName, ResourceName};
    use crate::state::resource::{
        AuthMode, FieldState, FieldVariant, PayloadShape, ResourceState, TopicScope, Verb,
        VerbState, WsEventsState, RESOURCE_SCHEMA_VERSION,
    };
    use crate::state::{save_app, save_resource, AppState};
    use indexmap::IndexMap;
    use std::collections::BTreeSet;
    use tempfile::tempdir;

    fn synthetic_resource(
        name: &str,
        shape: PayloadShape,
        scope: TopicScope,
    ) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let mut variants = BTreeSet::new();
        variants.insert(FieldVariant::Db);
        variants.insert(FieldVariant::Public);
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: crate::state::names::SqlType::new("BIGSERIAL"),
                variants,
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new(name),
            fields,
            verbs,
            ws_events: Some(WsEventsState {
                trigger_columns: BTreeSet::new(),
                payload_shape: shape,
                topic_scope: scope,
            }),
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
    fn emits_global_publisher_with_full_public_row() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        let resource =
            synthetic_resource("users", PayloadShape::Public, TopicScope::Global);
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut prog = NullProgress;
        let report = run(root, &mut sink, &mut prog).expect("run ws_topics");

        let target = root
            .join("src/transport/ws/generated/users.rs")
            .canonicalize()
            .expect("target exists");
        assert!(report
            .written
            .iter()
            .any(|p| p.canonicalize().ok().as_deref() == Some(target.as_path())));

        let body = fs::read_to_string(&target).expect("read users.rs");
        assert!(body.contains("pub const TOPIC: &str = \"users/all\";"));
        assert!(body.contains("pub enum Event"));
        assert!(body.contains("Created(UserPublic)"));
        assert!(body.contains("Updated(UserPublic)"));
        assert!(body.contains("Deleted { id: i64 }"));
        assert!(body.contains("pub fn publish_created("));
        assert!(body.contains("pub fn publish_updated("));
        assert!(body.contains("pub fn publish_deleted("));
        assert!(body.contains("use crate::structs::generated::users::UserPublic;"));

        let barrel = fs::read_to_string(root.join("src/transport/ws/generated/mod.rs"))
            .expect("read mod.rs");
        assert!(barrel.contains("pub mod users;"));
    }

    #[test]
    fn emits_scoped_publisher_with_id_only_payload() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        let resource = synthetic_resource(
            "orders",
            PayloadShape::IdOnly,
            TopicScope::ScopedTo(AuthScopeField::new("customer_id")),
        );
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut prog = NullProgress;
        run(root, &mut sink, &mut prog).expect("run ws_topics");

        let body = fs::read_to_string(root.join("src/transport/ws/generated/orders.rs"))
            .expect("read orders.rs");
        assert!(body.contains(
            "pub const TOPIC: &str = \"orders/by-customer_id/{customer_id}\";"
        ));
        assert!(body.contains("Created { id: i64 }"));
        assert!(body.contains("customer_id: i64"));
        assert!(body.contains("TOPIC.replace(\"{customer_id}\", &customer_id.to_string())"));
    }

    #[test]
    fn skips_resources_without_ws_events() {
        let tmp = tempdir().expect("tempdir");
        let root = tmp.path();
        let mut resource =
            synthetic_resource("posts", PayloadShape::Public, TopicScope::Global);
        resource.ws_events = None;
        seed_project(root, &[resource]);

        let mut sink = NullSink;
        let mut prog = NullProgress;
        let report = run(root, &mut sink, &mut prog).expect("run ws_topics");

        assert!(report.written.is_empty());
        assert!(!root
            .join("src/transport/ws/generated/posts.rs")
            .exists());
    }

    #[test]
    fn topic_template_per_row_uses_id_placeholder() {
        let tpl = topic_template("widgets", &TopicScope::PerRow);
        assert_eq!(tpl, "widgets/by-id/{id}");
    }
}
