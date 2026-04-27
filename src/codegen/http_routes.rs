use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state::{GenLevel, ResourceState, Verb};

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "http routes generation";

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let all_resources = match ir_loader::load_resource_states(project_root) {
        Ok(v) => v,
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_LABEL, &reason);
            sink.error(format!("{}: {}", STEP_LABEL, reason));
            return Err(err);
        }
    };

    let resources: Vec<ResourceState> = all_resources
        .into_iter()
        .filter(|r| r.gen_level >= GenLevel::Route)
        .collect();

    let mut report = EmitReport::default();
    let out_dir = generated_dir(project_root);
    fs::create_dir_all(&out_dir)?;

    for r in &resources {
        let target = out_dir.join(format!("{}.rs", r.name.as_str()));
        let marker = header::marker_for_resource(project_root, r.name.as_str())?;
        let body = format!("{}{}", marker, build_resource_file(r));
        write_file(&target, body.as_bytes(), &mut report)?;
        sink.info(format!("emitted {}", target.display()));
    }

    let barrel = out_dir.join("mod.rs");
    let barrel_marker = header::marker_for_app(project_root)?;
    let barrel_body = format!("{}{}", barrel_marker, build_barrel(&resources));
    write_file(&barrel, barrel_body.as_bytes(), &mut report)?;
    sink.info(format!("emitted {}", barrel.display()));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn write_file(target: &Path, bytes: &[u8], report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| {
        BlastError::Invalid(format!("http route target has no parent: {}", target.display()))
    })?;
    fs::create_dir_all(parent)?;
    fs::write(target, bytes)?;
    report.written.push(target.to_path_buf());
    Ok(())
}

fn generated_dir(project_root: &Path) -> PathBuf {
    project_root
        .join("src")
        .join("transport")
        .join("http")
        .join("generated")
}

fn build_resource_file(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let type_name = pascal_case(&singularize(table));

    let mut out = String::new();
    out.push_str("use axum::extract::{Path, Query, State};\n");
    out.push_str("use axum::http::StatusCode;\n");
    out.push_str("use axum::routing::{delete, get, patch, post};\n");
    out.push_str("use axum::{Json, Router};\n");
    out.push_str("use crate::Ctx;\n");
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str("use crate::transport::http::list_query::{ListQuery, ListResponse};\n");
    out.push('\n');
    out.push_str(&format!(
        "use crate::flows::generated::{table} as flow;\n",
        table = table,
    ));
    out.push_str(&format!(
        "use crate::structs::generated::{table}::{{{ty}Insertable, {ty}Patch, {ty}Public}};\n",
        table = table,
        ty = type_name,
    ));
    out.push('\n');

    for verb in r.verbs.keys() {
        out.push_str(&handler_for_verb(*verb, &type_name));
        out.push('\n');
    }

    out.push_str(&router_fn(r));
    out
}

fn handler_for_verb(verb: Verb, type_name: &str) -> String {
    match verb {
        Verb::List => list_handler(type_name),
        Verb::Get => get_handler(type_name),
        Verb::Create => create_handler(type_name),
        Verb::Update => update_handler(type_name),
        Verb::Delete => delete_handler(),
    }
}

fn list_handler(type_name: &str) -> String {
    format!(
        "pub async fn list(\n\
        \x20   State(ctx): State<Ctx>,\n\
        \x20   Query(params): Query<ListQuery>,\n\
        ) -> Result<Json<ListResponse<{ty}Public>>, MeltDown> {{\n\
        \x20   let result = flow::list::run(&ctx, params).await?;\n\
        \x20   Ok(Json(result))\n\
        }}\n",
        ty = type_name,
    )
}

fn get_handler(type_name: &str) -> String {
    format!(
        "pub async fn get_one(\n\
        \x20   State(ctx): State<Ctx>,\n\
        \x20   Path(id): Path<i64>,\n\
        ) -> Result<Json<{ty}Public>, MeltDown> {{\n\
        \x20   let result = flow::get::run(&ctx, id).await?;\n\
        \x20   Ok(Json(result))\n\
        }}\n",
        ty = type_name,
    )
}

fn create_handler(type_name: &str) -> String {
    format!(
        "pub async fn create(\n\
        \x20   State(ctx): State<Ctx>,\n\
        \x20   Json(input): Json<{ty}Insertable>,\n\
        ) -> Result<(StatusCode, Json<{ty}Public>), MeltDown> {{\n\
        \x20   let result = flow::create::run(&ctx, input).await?;\n\
        \x20   Ok((StatusCode::CREATED, Json(result)))\n\
        }}\n",
        ty = type_name,
    )
}

fn update_handler(type_name: &str) -> String {
    format!(
        "pub async fn update(\n\
        \x20   State(ctx): State<Ctx>,\n\
        \x20   Path(id): Path<i64>,\n\
        \x20   Json(patch): Json<{ty}Patch>,\n\
        ) -> Result<Json<{ty}Public>, MeltDown> {{\n\
        \x20   let result = flow::update::run(&ctx, id, patch).await?;\n\
        \x20   Ok(Json(result))\n\
        }}\n",
        ty = type_name,
    )
}

fn delete_handler() -> String {
    String::from(
        "pub async fn delete_one(\n\
        \x20   State(ctx): State<Ctx>,\n\
        \x20   Path(id): Path<i64>,\n\
        ) -> Result<StatusCode, MeltDown> {\n\
        \x20   flow::delete::run(&ctx, id).await?;\n\
        \x20   Ok(StatusCode::NO_CONTENT)\n\
        }\n",
    )
}

fn router_fn(r: &ResourceState) -> String {
    let has_list = r.verbs.contains_key(&Verb::List);
    let has_create = r.verbs.contains_key(&Verb::Create);
    let has_get = r.verbs.contains_key(&Verb::Get);
    let has_update = r.verbs.contains_key(&Verb::Update);
    let has_delete = r.verbs.contains_key(&Verb::Delete);

    let collection_chain = build_method_chain(&[
        (has_list, "get", "list"),
        (has_create, "post", "create"),
    ]);
    let item_chain = build_method_chain(&[
        (has_get, "get", "get_one"),
        (has_update, "patch", "update"),
        (has_delete, "delete", "delete_one"),
    ]);

    let mut out = String::new();
    out.push_str("pub fn router() -> Router<Ctx> {\n");
    out.push_str("    let mut router = Router::new();\n");
    match collection_chain {
        Some(chain) => out.push_str(&format!("    router = router.route(\"/\", {});\n", chain)),
        None => {}
    }
    match item_chain {
        Some(chain) => out.push_str(&format!("    router = router.route(\"/:id\", {});\n", chain)),
        None => {}
    }
    out.push_str("    router\n");
    out.push_str("}\n");
    out
}

fn build_method_chain(entries: &[(bool, &str, &str)]) -> Option<String> {
    let active: Vec<(&str, &str)> = entries
        .iter()
        .filter(|(enabled, _, _)| *enabled)
        .map(|(_, method, handler)| (*method, *handler))
        .collect();
    if active.is_empty() {
        return None;
    }
    let mut iter = active.iter();
    let head = match iter.next() {
        Some((method, handler)) => format!("{}({})", method, handler),
        None => return None,
    };
    let mut acc = head;
    for (method, handler) in iter {
        acc = format!("{}.{}({})", acc, method, handler);
    }
    Some(acc)
}

fn build_barrel(resources: &[ResourceState]) -> String {
    let mut names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    names.sort();

    let mut out = String::new();
    for name in &names {
        out.push_str(&format!("pub mod {};\n", name));
    }
    out.push('\n');
    out.push_str("use axum::Router;\n");
    out.push_str("use crate::Ctx;\n");
    out.push('\n');
    out.push_str("pub fn router() -> Router<Ctx> {\n");
    out.push_str("    let mut router = Router::new();\n");
    for name in &names {
        out.push_str(&format!(
            "    router = router.nest(\"/{name}\", {name}::router());\n",
            name = name,
        ));
    }
    out.push_str("    router\n");
    out.push_str("}\n");
    out
}

fn singularize(table: &str) -> String {
    for suffix in ["sses", "shes", "ches", "xes", "zes"] {
        match table.strip_suffix(suffix) {
            Some(stem) => return format!("{}{}", stem, &suffix[..suffix.len() - 2]),
            None => continue,
        }
    }
    match table.strip_suffix("ies") {
        Some(stem) => format!("{}y", stem),
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
    use crate::state::{
        AuthMode, FieldName, FieldState, FieldVariant, ResourceName, ResourceState, SqlType,
        Verb, VerbState,
    };
    use indexmap::IndexMap;
    use std::collections::BTreeSet;
    use tempfile::TempDir;

    fn write_state(project_root: &Path, table: &str) -> std::io::Result<()> {
        let resources_dir = project_root
            .join("storage")
            .join("blast")
            .join("state")
            .join("resources");
        fs::create_dir_all(&resources_dir)?;

        let mut field_variants = BTreeSet::new();
        field_variants.insert(FieldVariant::Db);
        field_variants.insert(FieldVariant::Public);
        let id_field = FieldState {
            sql_type: SqlType::new("BIGSERIAL"),
            variants: field_variants,
            nullable: false,
            primary_key: true,
            validators: BTreeSet::new(),
        };

        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(FieldName::new("id"), id_field);

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in [Verb::List, Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
            verbs.insert(
                v,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: None,
                },
            );
        }

        let mut resource = ResourceState::new(ResourceName::new(table));
        resource.fields = fields;
        resource.verbs = verbs;

        let state_dir = project_root.join("storage").join("blast").join("state");
        match crate::state::save_resource(&state_dir, &resource) {
            Ok(()) => Ok(()),
            Err(err) => Err(std::io::Error::other(err.to_string())),
        }
    }

    fn write_app_state(project_root: &Path) -> std::io::Result<()> {
        let state_dir = project_root.join("storage").join("blast").join("state");
        fs::create_dir_all(&state_dir)?;
        let app = crate::state::AppState::default();
        match crate::state::save_app(&state_dir, &app) {
            Ok(()) => Ok(()),
            Err(err) => Err(std::io::Error::other(err.to_string())),
        }
    }

    #[test]
    fn emits_per_resource_file_with_all_verbs() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_state(root, "users").expect("write resource state");
        write_app_state(root).expect("write app state");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        let report = run(root, &mut sink, &mut progress).expect("http routes generation");

        let resource_file = root
            .join("src/transport/http/generated/users.rs");
        let barrel = root.join("src/transport/http/generated/mod.rs");
        assert!(resource_file.exists(), "per-resource file missing");
        assert!(barrel.exists(), "barrel mod.rs missing");
        assert!(report.written.contains(&resource_file));
        assert!(report.written.contains(&barrel));

        let body = fs::read_to_string(&resource_file).expect("read resource file");
        assert!(body.contains("pub async fn list("), "list handler missing");
        assert!(body.contains("pub async fn get_one("), "get handler missing");
        assert!(body.contains("pub async fn create("), "create handler missing");
        assert!(body.contains("pub async fn update("), "update handler missing");
        assert!(body.contains("pub async fn delete_one("), "delete handler missing");
        assert!(
            body.contains("pub fn router() -> Router<Ctx>"),
            "router fn missing",
        );
        assert!(
            body.contains(".route(\"/\","),
            "collection route missing",
        );
        assert!(
            body.contains(".route(\"/:id\","),
            "item route missing",
        );

        let barrel_body = fs::read_to_string(&barrel).expect("read barrel");
        assert!(barrel_body.contains("pub mod users;"), "barrel module missing");
        assert!(
            barrel_body.contains(".nest(\"/users\", users::router())"),
            "barrel nest missing",
        );
    }

    #[test]
    fn skips_unspecified_verbs() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();

        let resources_dir = root
            .join("storage/blast/state/resources");
        fs::create_dir_all(&resources_dir).expect("mkdir");

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        let mut resource = ResourceState::new(ResourceName::new("logs"));
        resource.verbs = verbs;

        let state_dir = root.join("storage/blast/state");
        crate::state::save_resource(&state_dir, &resource).expect("save resource");
        write_app_state(root).expect("write app state");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("http routes generation");

        let body = fs::read_to_string(root.join("src/transport/http/generated/logs.rs"))
            .expect("read");
        assert!(body.contains("pub async fn list("));
        assert!(!body.contains("pub async fn create("));
        assert!(!body.contains("pub async fn delete_one("));
        assert!(!body.contains(".route(\"/:id\""), "no item route when no item verbs");
    }
}
