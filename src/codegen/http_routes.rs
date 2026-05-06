use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{CrankPolicy, FieldKind, GenLevel, ResourceState, SessionFieldRef, Verb},
};

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "http routes generation";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
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

    let resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Route).collect();

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

    let barrel_marker = header::marker_for_app(project_root)?;

    let barrel = out_dir.join("mod.rs");
    let barrel_body = format!("{}{}", barrel_marker, build_mod_rs(&resources));
    write_file(&barrel, barrel_body.as_bytes(), &mut report)?;
    sink.info(format!("emitted {}", barrel.display()));

    let router_file = out_dir.join("router.rs");
    let router_body = format!("{}{}", barrel_marker, build_router_rs(&resources));
    write_file(&router_file, router_body.as_bytes(), &mut report)?;
    sink.info(format!("emitted {}", router_file.display()));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn write_file(target: &Path, bytes: &[u8], report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("http route target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    fs::write(target, bytes)?;
    report.written.push(target.to_path_buf());
    Ok(())
}

fn generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("transport").join("http").join("generated")
}

fn build_resource_file(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let type_name = pascal_case(&singularize(table));

    let create_emits = verb_emits_rest(r, Verb::Create);
    let update_emits = verb_emits_rest(r, Verb::Update);
    let needs_validator_import = r.gen_level >= crate::state::GenLevel::Types && (create_emits || update_emits);

    let routing_head_idents = collect_routing_head_idents(r);

    let mut out = String::new();
    out.push_str("use axum::extract::Path;\n");
    out.push_str("use axum::http::StatusCode;\n");
    if !routing_head_idents.is_empty() {
        let body = match routing_head_idents.len() {
            1 => routing_head_idents[0].to_string(),
            _other => format!("{{{}}}", routing_head_idents.join(", ")),
        };
        out.push_str(&format!("use axum::routing::{};\n", body));
    }
    out.push_str("use axum::{Extension, Json, Router};\n");
    out.push_str("use crate::Ctx;\n");
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str("use crate::structs::list_query::{ListQuery, ListResponse};\n");
    out.push('\n');
    out.push_str(&format!("use crate::flows::generated::{table} as flow;\n", table = table,));
    let mut ty_imports: Vec<String> = vec![format!("{ty}Public", ty = type_name)];
    if create_emits {
        ty_imports.push(format!("{ty}Insertable", ty = type_name));
    }
    if update_emits {
        ty_imports.push(format!("{ty}Patch", ty = type_name));
    }
    out.push_str(&format!("use crate::structs::generated::{table}::{{{names}}};\n", table = table, names = ty_imports.join(", "),));
    if needs_validator_import {
        out.push_str("use crate::structs::vendored::validators::Validate;\n");
    }
    out.push('\n');

    for verb in r.verbs.keys() {
        if !verb_emits_rest(r, *verb) {
            continue;
        }
        out.push_str(&handler_for_verb(*verb, r, &type_name, table, needs_validator_import));
        out.push('\n');
    }

    out.push_str(&router_fn(r));
    out
}

fn verb_emits_rest(r: &ResourceState, verb: Verb) -> bool {
    match r.verbs.get(&verb) {
        Some(state) => state.emit_rest_api,
        None => false, // allow: absent verb declaration means no REST emission for this verb
    }
}

fn handler_for_verb(verb: Verb, r: &ResourceState, type_name: &str, table: &str, validators_enabled: bool) -> String {
    match verb {
        Verb::List => list_handler(type_name),
        Verb::Get => get_handler(type_name),
        Verb::Create => create_handler(r, type_name, table, validators_enabled),
        Verb::Update => update_handler(type_name, table, validators_enabled),
        Verb::Delete => delete_handler(),
    }
}

fn session_injections(r: &ResourceState) -> String {
    let mut out = String::new();
    for (name, field) in &r.fields {
        let scope = match &field.kind {
            FieldKind::FromSession(scope) => scope,
            _other => continue,
        };
        let accessor = match scope {
            SessionFieldRef::UserId => "user_id",
            SessionFieldRef::SessionId => "session_id",
        };
        out.push_str(&format!(
            "\x20   input.{name} = ctx.require_session()?.{accessor};\n",
            name = name.as_str(),
            accessor = accessor,
        ));
    }
    out
}

fn list_handler(type_name: &str) -> String {
    format!(
        "pub async fn list(\n\x20   Extension(ctx): Extension<Ctx>,\n\x20   params: ListQuery,\n) -> Result<Json<ListResponse<{ty}Public>>, MeltDown> {{\n\x20   let result = flow::list::run(&ctx, \
         params).await?;\n\x20   Ok(Json(result))\n}}\n",
        ty = type_name,
    )
}

fn get_handler(type_name: &str) -> String {
    format!(
        "pub async fn get_one(\n\x20   Extension(ctx): Extension<Ctx>,\n\x20   Path(id): Path<i64>,\n) -> Result<Json<{ty}Public>, MeltDown> {{\n\x20   let result = flow::get::run(&ctx, id).await?;\n\x20   \
         Ok(Json(result))\n}}\n",
        ty = type_name,
    )
}

fn create_handler(r: &ResourceState, type_name: &str, _table: &str, validators_enabled: bool) -> String {
    let validator_call = if validators_enabled {
        "    input.check()?;\n".to_string()
    } else {
        String::new()
    };
    let injections = session_injections(r);
    let input_pat = if injections.is_empty() {
        "input"
    } else {
        "mut input"
    };
    format!(
        "pub async fn create(\n\x20   Extension(ctx): Extension<Ctx>,\n\x20   Json({input_pat}): Json<{ty}Insertable>,\n) -> Result<(StatusCode, Json<{ty}Public>), MeltDown> {{\n{injections}{validator}\x20   let result = flow::create::run(&ctx, \
         input).await?;\n\x20   Ok((StatusCode::CREATED, Json(result)))\n}}\n",
        input_pat = input_pat,
        ty = type_name,
        validator = validator_call,
        injections = injections,
    )
}

fn update_handler(type_name: &str, _table: &str, validators_enabled: bool) -> String {
    let validator_call = if validators_enabled {
        "    patch.check()?;\n".to_string()
    } else {
        String::new()
    };
    format!(
        "pub async fn update(\n\x20   Extension(ctx): Extension<Ctx>,\n\x20   Path(id): Path<i64>,\n\x20   Json(patch): Json<{ty}Patch>,\n) -> Result<Json<{ty}Public>, MeltDown> {{\n{validator}\x20   let result = \
         flow::update::run(&ctx, id, patch).await?;\n\x20   Ok(Json(result))\n}}\n",
        ty = type_name,
        validator = validator_call,
    )
}

fn delete_handler() -> String {
    String::from(
        "pub async fn delete_one(\n\x20   Extension(ctx): Extension<Ctx>,\n\x20   Path(id): Path<i64>,\n) -> Result<StatusCode, MeltDown> {\n\x20   flow::delete::run(&ctx, id).await?;\n\x20   Ok(StatusCode::NO_CONTENT)\n}\n",
    )
}

fn router_fn(r: &ResourceState) -> String {
    let has_list = verb_emits_rest(r, Verb::List);
    let has_create = verb_emits_rest(r, Verb::Create);
    let has_get = verb_emits_rest(r, Verb::Get);
    let has_update = verb_emits_rest(r, Verb::Update);
    let has_delete = verb_emits_rest(r, Verb::Delete);

    let collection_chain = build_method_chain(&[(has_list, "get", "list"), (has_create, "post", "create")]);
    let item_chain = build_method_chain(&[(has_get, "get", "get_one"), (has_update, "patch", "update"), (has_delete, "delete", "delete_one")]);

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

fn collect_routing_head_idents(r: &ResourceState) -> Vec<&'static str> {
    let has_list = verb_emits_rest(r, Verb::List);
    let has_create = verb_emits_rest(r, Verb::Create);
    let has_get = verb_emits_rest(r, Verb::Get);
    let has_update = verb_emits_rest(r, Verb::Update);
    let has_delete = verb_emits_rest(r, Verb::Delete);

    let mut heads: Vec<&'static str> = Vec::new();
    let collection = [(has_list, "get"), (has_create, "post")];
    let item = [(has_get, "get"), (has_update, "patch"), (has_delete, "delete")];
    for chain in [&collection[..], &item[..]] {
        for (enabled, method) in chain {
            if *enabled {
                heads.push(method);
                break;
            }
        }
    }
    heads.sort();
    heads.dedup();
    heads
}

fn build_method_chain(entries: &[(bool, &str, &str)]) -> Option<String> {
    let active: Vec<(&str, &str)> = entries.iter().filter(|(enabled, _, _)| *enabled).map(|(_, method, handler)| (*method, *handler)).collect();
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

fn build_mod_rs(resources: &[ResourceState]) -> String {
    let mut names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    names.sort();

    let mut out = String::new();
    for name in &names {
        out.push_str(&format!("pub mod {};\n", name));
    }
    out.push_str("pub mod router;\n");
    out.push('\n');
    out.push_str("pub use router::router;\n");
    out
}

fn build_router_rs(resources: &[ResourceState]) -> String {
    let mut names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    names.sort();

    let mut out = String::new();
    out.push_str("use axum::Router;\n");
    out.push_str("use crate::Ctx;\n");
    out.push('\n');
    out.push_str("pub fn router() -> Router<Ctx> {\n");
    out.push_str("    let mut router = Router::new();\n");
    for name in &names {
        out.push_str(&format!("    router = router.nest(\"/{name}\", super::{name}::router());\n", name = name,));
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
    use std::collections::BTreeSet;

    use indexmap::IndexMap;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        io::null::{NullProgress, NullSink},
        state::{AuthMode, FieldName, FieldState, FieldVariant, ResourceName, ResourceState, SqlType, Verb, VerbState},
    };

    fn write_state(project_root: &Path, table: &str) -> std::io::Result<()> {
        let resources_dir = project_root.join("storage").join("blast").join("state").join("resources");
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
        
            kind: Default::default(),
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
                    emit_rest_api: true,
                    emit_html_page: true,
                                    crank_policy: CrankPolicy::None,
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

        let resource_file = root.join("src/transport/http/generated/users.rs");
        let barrel = root.join("src/transport/http/generated/mod.rs");
        let router_file = root.join("src/transport/http/generated/router.rs");
        assert!(resource_file.exists(), "per-resource file missing");
        assert!(barrel.exists(), "barrel mod.rs missing");
        assert!(router_file.exists(), "router.rs missing");
        assert!(report.written.contains(&resource_file));
        assert!(report.written.contains(&barrel));
        assert!(report.written.contains(&router_file));

        let body = fs::read_to_string(&resource_file).expect("read resource file");
        assert!(body.contains("pub async fn list("), "list handler missing");
        assert!(body.contains("pub async fn get_one("), "get handler missing");
        assert!(body.contains("pub async fn create("), "create handler missing");
        assert!(body.contains("pub async fn update("), "update handler missing");
        assert!(body.contains("pub async fn delete_one("), "delete handler missing");
        assert!(body.contains("pub fn router() -> Router<Ctx>"), "router fn missing",);
        assert!(body.contains(".route(\"/\","), "collection route missing",);
        assert!(body.contains(".route(\"/:id\","), "item route missing",);

        let barrel_body = fs::read_to_string(&barrel).expect("read barrel");
        assert!(barrel_body.contains("pub mod users;"), "barrel module missing");
        assert!(barrel_body.contains("pub mod router;"), "barrel router module missing");
        assert!(barrel_body.contains("pub use router::router;"), "barrel re-export missing");

        let router_body = fs::read_to_string(&router_file).expect("read router.rs");
        assert!(router_body.contains("pub fn router() -> Router<Ctx>"), "top router fn missing");
        assert!(router_body.contains(".nest(\"/users\", super::users::router())"), "nest missing in router.rs",);
    }

    #[test]
    fn create_handler_calls_validator_before_flow() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_state(root, "users").expect("write resource state");
        write_app_state(root).expect("write app state");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("http routes generation");

        let resource_file = root.join("src/transport/http/generated/users.rs");
        let body = fs::read_to_string(&resource_file).expect("read");
        let validator_call = "input.check()?;";
        let flow_call = "flow::create::run(&ctx, input)";
        let validator_pos = body.find(validator_call).expect("validator call must be emitted");
        let flow_pos = body.find(flow_call).expect("flow call must be emitted");
        assert!(validator_pos < flow_pos, "validator must come BEFORE flow call: validator@{} flow@{} body=\n{}", validator_pos, flow_pos, body);
    }

    #[test]
    fn update_handler_calls_validator_before_flow() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_state(root, "users").expect("write resource state");
        write_app_state(root).expect("write app state");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("http routes generation");

        let resource_file = root.join("src/transport/http/generated/users.rs");
        let body = fs::read_to_string(&resource_file).expect("read");
        let validator_call = "patch.check()?;";
        let flow_call = "flow::update::run(&ctx, id, patch)";
        let validator_pos = body.find(validator_call).expect("validator call must be emitted");
        let flow_pos = body.find(flow_call).expect("flow call must be emitted");
        assert!(validator_pos < flow_pos, "validator must come BEFORE flow call");
    }

    #[test]
    fn validate_trait_imported_when_create_or_update_present() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_state(root, "users").expect("write resource state");
        write_app_state(root).expect("write app state");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("http routes generation");

        let resource_file = root.join("src/transport/http/generated/users.rs");
        let body = fs::read_to_string(&resource_file).expect("read");
        assert!(body.contains("use crate::structs::vendored::validators::Validate;"), "must import Validate trait; got: {body}");
    }

    #[test]
    fn emit_rest_api_false_skips_handler_and_route() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();

        let resources_dir = root.join("storage/blast/state/resources");
        fs::create_dir_all(&resources_dir).expect("mkdir");

        let mut field_variants = BTreeSet::new();
        field_variants.insert(FieldVariant::Db);
        field_variants.insert(FieldVariant::Public);
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("BIGSERIAL"),
                variants: field_variants,
                nullable: false,
                primary_key: true,
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
                            crank_policy: CrankPolicy::None,
            },
        );
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
                emit_rest_api: false,
                emit_html_page: true,
                            crank_policy: CrankPolicy::None,
            },
        );

        let mut resource = ResourceState::new(ResourceName::new("widgets"));
        resource.fields = fields;
        resource.verbs = verbs;

        let state_dir = root.join("storage/blast/state");
        crate::state::save_resource(&state_dir, &resource).expect("save resource");
        write_app_state(root).expect("write app state");

        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(root, &mut sink, &mut progress).expect("http routes generation");

        let body = fs::read_to_string(root.join("src/transport/http/generated/widgets.rs")).expect("read");
        assert!(body.contains("pub async fn list("), "list handler must emit when emit_rest_api: true");
        assert!(!body.contains("pub async fn get_one("), "get handler must NOT emit when emit_rest_api: false");
        assert!(!body.contains(".route(\"/:id\""), "item route must NOT emit when no item REST verbs");
    }

    #[test]
    fn skips_unspecified_verbs() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();

        let resources_dir = root.join("storage/blast/state/resources");
        fs::create_dir_all(&resources_dir).expect("mkdir");

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
                            crank_policy: CrankPolicy::None,
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

        let body = fs::read_to_string(root.join("src/transport/http/generated/logs.rs")).expect("read");
        assert!(body.contains("pub async fn list("));
        assert!(!body.contains("pub async fn create("));
        assert!(!body.contains("pub async fn delete_one("));
        assert!(!body.contains(".route(\"/:id\""), "no item route when no item verbs");
    }
}
