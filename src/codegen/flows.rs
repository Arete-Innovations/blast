use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    codegen::{header, ir_loader},
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state::{AuthMode, GenLevel, ResourceState, Verb},
};

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "flows: emit per-resource stubs";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let all_resources = ir_loader::load_resource_states(project_root)?;
    let resources: Vec<ResourceState> = all_resources.into_iter().filter(|r| r.gen_level >= GenLevel::Route).collect();
    let mut report = EmitReport::default();

    if resources.is_empty() {
        sink.info("flows: no resources declared; nothing to emit");
        progress.step_done(STEP_LABEL);
        return Ok(report);
    }

    let total = resources.len() as u64;
    for (idx, resource) in resources.iter().enumerate() {
        emit_resource(project_root, resource, &mut report)?;
        progress.tick(idx as u64 + 1, total);
    }

    let out_dir = flows_generated_dir(project_root);
    let barrel_target = out_dir.join("mod.rs");
    let barrel_marker = header::marker_for_app(project_root)?;
    let tables: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    write_file(&barrel_target, &format!("{}{}", barrel_marker, render_top_barrel(&tables)), &mut report)?;

    sink.info(format!("flows: {} file(s) written across {} resource(s)", report.written.len(), resources.len()));

    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn emit_resource(project_root: &Path, resource: &ResourceState, report: &mut EmitReport) -> BlastResult<()> {
    let table = resource.name.as_str();
    let resource_dir = flows_generated_dir(project_root).join(table);
    fs::create_dir_all(&resource_dir)?;

    let marker = header::marker_for_resource(project_root, table)?;

    let verbs: Vec<Verb> = resource.verbs.keys().copied().collect();

    write_file(&resource_dir.join("mod.rs"), &format!("{}{}", marker, barrel_body(&verbs)), report)?;

    for verb in &verbs {
        let auth = match resource.verbs.get(verb) {
            Some(verb_state) => &verb_state.auth,
            None => {
                return Err(BlastError::Invalid(format!("verb {:?} vanished from resource {} between iter and lookup", verb, table)));
            }
        };
        let body = verb_stub_body(table, *verb, auth);
        write_file(&resource_dir.join(format!("{}.rs", verb_module(*verb))), &format!("{}{}", marker, body), report)?;
    }

    Ok(())
}

fn flows_generated_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("flows").join("generated")
}

fn write_file(target: &Path, body: &str, report: &mut EmitReport) -> BlastResult<()> {
    let parent = target.parent().ok_or_else(|| BlastError::Invalid(format!("flow target has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    let mut file = fs::File::create(target)?;
    file.write_all(body.as_bytes())?;
    report.written.push(target.to_path_buf());
    Ok(())
}

fn barrel_body(verbs: &[Verb]) -> String {
    let mut out = String::new();
    for verb in verbs {
        out.push_str(&format!("pub mod {};\n", verb_module(*verb)));
    }
    out
}

fn render_top_barrel(tables: &[&str]) -> String {
    let mut sorted: Vec<&str> = tables.to_vec();
    sorted.sort();
    let mut out = String::new();
    for t in sorted {
        out.push_str(&format!("pub mod {t};\n"));
    }
    out
}

fn verb_module(verb: Verb) -> &'static str {
    match verb {
        Verb::List => "list",
        Verb::Get => "get",
        Verb::Create => "create",
        Verb::Update => "update",
        Verb::Delete => "delete",
    }
}

fn verb_stub_body(table: &str, verb: Verb, auth: &AuthMode) -> String {
    let auth_block = auth_check_block(auth);
    let singular = singularize(table);
    let type_stem = pascal_case(&singular);
    let (args_sig, ret_ty, routine_call) = verb_signature(verb, &type_stem);

    let mut out = String::new();
    out.push_str("use crate::crank::Crank;\n");
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str("use crate::routines;\n");
    if matches!(auth, AuthMode::AdminOnly | AuthMode::Roles(_)) {
        out.push_str("use crate::structs::UserRole;\n");
    }
    if !matches!(verb, Verb::Delete) {
        out.push_str("use crate::structs::generated::");
        out.push_str(table);
        out.push_str("::*;\n");
    }
    out.push_str("use crate::Ctx;\n\n");

    out.push_str(&format!("pub async fn run(ctx: &Ctx{args_sig}) -> Result<{ret_ty}, MeltDown> {{\n"));
    if !auth_block.is_empty() {
        for line in auth_block.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(&format!("    Crank::none().run(|| routines::generated::{table}::{routine_call}).await\n"));
    out.push_str("}\n");

    out
}

fn verb_signature(verb: Verb, type_stem: &str) -> (String, String, String) {
    match verb {
        Verb::List => (
            String::from(", query: crate::structs::list_query::ListQuery"),
            format!("crate::structs::list_query::ListResponse<{type_stem}Public>"),
            String::from("list::run(ctx, query.clone())"),
        ),
        Verb::Get => (String::from(", id: i64"), format!("{type_stem}Public"), String::from("get::run(ctx, id)")),
        Verb::Create => (format!(", input: {type_stem}Insertable"), format!("{type_stem}Public"), String::from("create::run(ctx, input.clone())")),
        Verb::Update => (format!(", id: i64, patch: {type_stem}Patch"), format!("{type_stem}Public"), String::from("update::run(ctx, id, patch.clone())")),
        Verb::Delete => (String::from(", id: i64"), String::from("()"), String::from("delete::run(ctx, id)")),
    }
}

fn auth_check_block(auth: &AuthMode) -> String {
    match auth {
        AuthMode::Public => String::new(),
        AuthMode::AuthRequired => String::from("ctx.require_session()?;"),
        AuthMode::AdminOnly => String::from("ctx.require_any(&[UserRole::Admin])?;"),
        AuthMode::ScopedTo(field) => {
            let mut out = String::new();
            out.push_str("ctx.require_session()?;\n");
            out.push_str(&format!("// TODO(catalyst): verify resource.{} matches session.user_id (needs catalyst scope-check API)", field.as_str()));
            out
        }
        AuthMode::Roles(roles) => {
            let mut sorted: Vec<&str> = roles.iter().map(|s| s.as_str()).collect();
            sorted.sort();
            let formatted = sorted.iter().map(|r| format!("UserRole::{}", pascal_case(r))).collect::<Vec<_>>().join(", ");
            format!("ctx.require_any(&[{formatted}])?;")
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
    use std::{collections::BTreeSet, fs as stdfs};

    use indexmap::IndexMap;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        io::events::{ProgressEvent, SinkEvent},
        state::{resource::RESOURCE_SCHEMA_VERSION, AuthMode, FieldName, FieldState, FieldVariant, ResourceName, ResourceState, SqlType, Verb, VerbState},
    };

    struct CapturingSink {
        events: Vec<SinkEvent>,
    }
    impl Sink for CapturingSink {
        fn emit(&mut self, event: SinkEvent) {
            self.events.push(event);
        }
    }

    struct CapturingProgress {
        events: Vec<ProgressEvent>,
    }
    impl Progress for CapturingProgress {
        fn emit(&mut self, event: ProgressEvent) {
            self.events.push(event);
        }
    }

    fn write_resource_ron(project_root: &Path, name: &str) {
        let resources_dir = project_root.join("storage/blast/state/resources");
        stdfs::create_dir_all(&resources_dir).expect("mkdir resources");
        let state_dir = project_root.join("storage/blast/state");
        let app = crate::state::AppState::default();
        crate::state::io::save_app(&state_dir, &app).expect("save app");
        let mut variants = BTreeSet::new();
        variants.insert(FieldVariant::Public);
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("BIGINT"),
                variants: variants.clone(),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::AuthRequired,
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
                auth: AuthMode::AdminOnly,
                list_options: None,
                emit_rest_api: true,
                emit_html_page: true,
            },
        );
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::Roles({
                    let mut s = BTreeSet::new();
                    s.insert(String::from("admin"));
                    s.insert(String::from("editor"));
                    s
                }),
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
        let resource = ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new(name),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: std::collections::BTreeMap::new(),
            gen_level: crate::state::GenLevel::default(),
        };
        let path = resources_dir.join(format!("{}.ron", name));
        let body = ron::ser::to_string_pretty(&resource, ron::ser::PrettyConfig::default()).expect("serialize resource");
        stdfs::write(&path, body).expect("write resource ron");
    }

    #[test]
    fn emits_barrel_and_verb_files_with_headers() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_resource_ron(root, "users");

        let mut sink = CapturingSink { events: Vec::new() };
        let mut progress = CapturingProgress { events: Vec::new() };
        let report = run(root, &mut sink, &mut progress).expect("flows codegen ok");

        let users_dir = root.join("src/flows/generated/users");
        let expected = [
            users_dir.join("mod.rs"),
            users_dir.join("list.rs"),
            users_dir.join("get.rs"),
            users_dir.join("create.rs"),
            users_dir.join("update.rs"),
            users_dir.join("delete.rs"),
        ];
        for p in &expected {
            assert!(p.exists(), "missing emitted file: {}", p.display());
            let body = stdfs::read_to_string(p).expect("read emitted file");
            assert!(body.starts_with("// AUTO-GENERATED from "), "missing marker in {}", p.display());
            assert!(body.contains("storage/blast/state/resources/users.ron"), "marker should reference state path in {}", p.display());
        }
        assert_eq!(report.written.len(), expected.len() + 1, "expected per-resource files + top barrel");

        let list_body = stdfs::read_to_string(users_dir.join("list.rs")).expect("read list");
        assert!(list_body.contains("ctx.require_session()?;"), "AuthRequired should emit require_session check");
        assert!(
            list_body.contains("Crank::none().run(|| routines::generated::users::list::run(ctx, query.clone()))"),
            "list flow must wrap routine in Crank: {}",
            list_body
        );

        let get_body = stdfs::read_to_string(users_dir.join("get.rs")).expect("read get");
        assert!(!get_body.contains("require_session"), "Public verb must not emit auth check");
        assert!(get_body.contains("routines::generated::users::get::run(ctx, id)"), "get must call routine: {}", get_body);

        let create_body = stdfs::read_to_string(users_dir.join("create.rs")).expect("read create");
        assert!(create_body.contains("ctx.require_any(&[UserRole::Admin])?;"), "AdminOnly should emit require_any(&[UserRole::Admin])");
        assert!(create_body.contains("use crate::structs::UserRole;"), "AdminOnly must import Role");
        assert!(create_body.contains("routines::generated::users::create::run(ctx, input.clone())"));

        let update_body = stdfs::read_to_string(users_dir.join("update.rs")).expect("read update");
        assert!(
            update_body.contains("ctx.require_any(&[UserRole::Admin, UserRole::Editor])?;"),
            "Roles should emit sorted Role enum variants, got: {}",
            update_body
        );
        assert!(update_body.contains("routines::generated::users::update::run(ctx, id, patch.clone())"));

        let delete_body = stdfs::read_to_string(users_dir.join("delete.rs")).expect("read delete");
        assert!(delete_body.contains("routines::generated::users::delete::run(ctx, id)"));

        let mod_body = stdfs::read_to_string(users_dir.join("mod.rs")).expect("read mod");
        for v in ["list", "get", "create", "update", "delete"] {
            assert!(mod_body.contains(&format!("pub mod {};", v)), "barrel missing pub mod {}", v);
        }

        let top_barrel = stdfs::read_to_string(root.join("src/flows/generated/mod.rs")).expect("read top barrel");
        assert!(top_barrel.contains("pub mod users;"), "top barrel must list users");
        assert!(top_barrel.starts_with("// AUTO-GENERATED from "));
    }

    #[test]
    fn empty_state_emits_no_files() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        stdfs::create_dir_all(root.join("storage/blast/state/resources")).expect("mkdir state");

        let mut sink = CapturingSink { events: Vec::new() };
        let mut progress = CapturingProgress { events: Vec::new() };
        let report = run(root, &mut sink, &mut progress).expect("flows codegen ok on empty");
        assert!(report.written.is_empty());
        assert!(!root.join("src/flows/generated").exists());
    }

    #[test]
    fn auth_check_block_handles_all_modes() {
        assert!(auth_check_block(&AuthMode::Public).is_empty());
        assert_eq!(auth_check_block(&AuthMode::AuthRequired), "ctx.require_session()?;");
        assert_eq!(auth_check_block(&AuthMode::AdminOnly), "ctx.require_any(&[UserRole::Admin])?;");
        let mut roles = BTreeSet::new();
        roles.insert(String::from("zeta"));
        roles.insert(String::from("alpha_team"));
        assert_eq!(auth_check_block(&AuthMode::Roles(roles)), "ctx.require_any(&[UserRole::AlphaTeam, UserRole::Zeta])?;");
        let scoped = auth_check_block(&AuthMode::ScopedTo(crate::state::AuthScopeField::new("owner_id")));
        assert!(scoped.contains("owner_id"));
        assert!(scoped.contains("require_session()?;"));
    }
}
