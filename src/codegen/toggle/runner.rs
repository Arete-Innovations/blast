use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::{header, ir_loader, structs::naming::type_stem_for_resource};
use crate::error::BlastResult;
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state::{FieldKind, FieldName, GenLevel, ResourceState, SessionFieldRef};

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
}

const STEP_LABEL: &str = "toggle codegen";

pub fn run(project_root: &Path, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);

    let all = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(err) => {
            progress.step_fail(STEP_LABEL, &err.to_string());
            return Err(err);
        }
    };

    let mut report = EmitReport::default();
    let mut emitted_count = 0usize;

    for r in &all {
        if r.toggle_endpoint.is_none() {
            continue;
        }
        if r.gen_level < GenLevel::Route {
            continue;
        }
        emit_resource_toggle(project_root, r, &mut report)?;
        emitted_count += 1;
        sink.info(format!("emitted toggle for {}", r.name.as_str()));
    }

    sink.info(format!("{STEP_LABEL}: {} resource(s) toggled", emitted_count));
    progress.step_done(STEP_LABEL);
    Ok(report)
}

fn emit_resource_toggle(project_root: &Path, r: &ResourceState, report: &mut EmitReport) -> BlastResult<()> {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let scope = match r.toggle_endpoint.as_ref() {
        Some(t) => t.scope_field.as_str().to_string(),
        None => return Ok(()), // allow: filter above guarantees Some, defensive bail
    };
    let session_fields = collect_session_fields(r);
    let marker = header::marker_for_resource(project_root, table)?;

    write(
        project_root.join(format!("src/structs/generated/{table}_toggle.rs")),
        format!("{marker}{}", render_struct(&stem)),
        report,
    )?;
    write(
        project_root.join(format!("src/models/generated/{table}_toggle.rs")),
        format!("{marker}{}", render_model(table, &stem, &scope, &session_fields)),
        report,
    )?;
    write(
        project_root.join(format!("src/routines/generated/{table}_toggle.rs")),
        format!("{marker}{}", render_routine(table, &stem, &scope, &session_fields)),
        report,
    )?;
    write(
        project_root.join(format!("src/flows/generated/{table}_toggle.rs")),
        format!("{marker}{}", render_flow(table, &stem, &scope, &session_fields)),
        report,
    )?;
    write(
        project_root.join(format!("src/transport/http/generated/{table}_toggle.rs")),
        format!("{marker}{}", render_http(table, &stem, &scope)),
        report,
    )?;
    write(
        project_root.join(format!("src/transport/leptos/data/generated/{table}_toggle.rs")),
        format!("{marker}{}", render_leptos_data(table, &stem, &scope)),
        report,
    )?;

    update_barrel(project_root.join("src/structs/generated/mod.rs"), table, report)?;
    update_barrel(project_root.join("src/models/generated/mod.rs"), table, report)?;
    update_barrel(project_root.join("src/routines/generated/mod.rs"), table, report)?;
    update_barrel(project_root.join("src/flows/generated/mod.rs"), table, report)?;
    update_barrel(project_root.join("src/transport/http/generated/mod.rs"), table, report)?;
    update_barrel(project_root.join("src/transport/leptos/data/generated/mod.rs"), table, report)?;

    update_http_router(project_root, table, report)?;

    Ok(())
}

fn collect_session_fields(r: &ResourceState) -> Vec<(FieldName, SessionFieldRef)> {
    let mut out = Vec::new();
    for (name, field) in &r.fields {
        if let FieldKind::FromSession(ref_) = &field.kind { // allow: pattern-match on enum variant
            out.push((name.clone(), *ref_));
        }
    }
    out
}

fn session_accessor(r: SessionFieldRef) -> &'static str {
    match r {
        SessionFieldRef::UserId => "user_id",
        SessionFieldRef::SessionId => "session_id",
    }
}

fn render_struct(stem: &str) -> String {
    format!(
        "use serde::{{Deserialize, Serialize}};\n\n#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct {stem}ToggleResp {{\n    pub active: bool,\n    pub count: i64,\n}}\n"
    )
}

fn render_model(table: &str, stem: &str, scope: &str, session: &[(FieldName, SessionFieldRef)]) -> String {
    let session_args: Vec<String> = session.iter().map(|(n, _)| format!("{}: i64", n.as_str())).collect();
    let session_filters: Vec<String> = session.iter().map(|(n, _)| format!(".filter(schema::{}.eq({}))", n.as_str(), n.as_str())).collect();
    let mut insert_values: Vec<String> = vec![format!("schema::{scope}.eq({scope})")];
    for (n, _) in session {
        insert_values.push(format!("schema::{}.eq({})", n.as_str(), n.as_str()));
    }
    let mut all_args: Vec<String> = vec![format!("{scope}: i64")];
    all_args.extend(session_args.iter().cloned());
    let args_joined = all_args.join(", ");
    let session_filters_joined = session_filters.join("\n        ");
    let insert_values_joined = insert_values.join(",\n            ");

    format!(
        r#"use crate::meltdown::MeltDown;
use crate::structs::generated::{table}_toggle::{stem}ToggleResp;

pub async fn run(
    conn: &mut ::diesel_async::AsyncPgConnection,
    {args_joined},
) -> ::std::result::Result<{stem}ToggleResp, MeltDown> {{
    use ::diesel::ExpressionMethods;
    use ::diesel::QueryDsl;
    use ::diesel_async::RunQueryDsl;
    use ::diesel::OptionalExtension;
    use crate::database::schema::{table}::dsl as schema;

    let existing: Option<i64> = schema::{table}
        .filter(schema::{scope}.eq({scope}))
        {session_filters_joined}
        .select(schema::id)
        .first::<i64>(conn)
        .await
        .optional()?;

    let active = match existing {{
        Some(id) => {{
            ::diesel::delete(schema::{table}.filter(schema::id.eq(id)))
                .execute(conn)
                .await?;
            false
        }}
        None => {{
            ::diesel::insert_into(schema::{table})
                .values((
                    {insert_values_joined},
                ))
                .execute(conn)
                .await?;
            true
        }}
    }};

    let count: i64 = schema::{table}
        .filter(schema::{scope}.eq({scope}))
        .count()
        .get_result::<i64>(conn)
        .await?;

    Ok({stem}ToggleResp {{ active, count }})
}}
"#
    )
}

fn render_routine(table: &str, stem: &str, scope: &str, session: &[(FieldName, SessionFieldRef)]) -> String {
    let session_args: Vec<String> = session.iter().map(|(n, _)| format!("{}: i64", n.as_str())).collect();
    let session_call_args: Vec<String> = session.iter().map(|(n, _)| n.as_str().to_string()).collect();
    let mut sig_args: Vec<String> = vec![format!("{scope}: i64")];
    sig_args.extend(session_args.iter().cloned());
    let mut model_args: Vec<String> = vec![scope.to_string()];
    model_args.extend(session_call_args.iter().cloned());
    let sig_joined = sig_args.join(", ");
    let model_args_joined = model_args.join(", ");

    format!(
        r#"use crate::structs::generated::{table}_toggle::{stem}ToggleResp;
use crate::meltdown::MeltDown;
use crate::Ctx;

pub async fn run(
    ctx: &Ctx,
    {sig_joined},
) -> ::std::result::Result<{stem}ToggleResp, MeltDown> {{
    let mut conn = ctx.conn().await?;
    crate::models::generated::{table}_toggle::run(&mut conn, {model_args_joined}).await
}}
"#
    )
}

fn render_flow(table: &str, stem: &str, scope: &str, session: &[(FieldName, SessionFieldRef)]) -> String {
    let session_lines: Vec<String> = session.iter().map(|(n, r)| format!("    let {} = session.{};", n.as_str(), session_accessor(*r))).collect();
    let session_call_args: Vec<String> = session.iter().map(|(n, _)| n.as_str().to_string()).collect();
    let mut routine_args: Vec<String> = vec![scope.to_string()];
    routine_args.extend(session_call_args.iter().cloned());
    let session_lines_joined = session_lines.join("\n");
    let routine_args_joined = routine_args.join(", ");

    format!(
        r#"use crate::crank::Crank;
use crate::meltdown::MeltDown;
use crate::structs::generated::{table}_toggle::{stem}ToggleResp;
use crate::Ctx;

pub async fn run(ctx: &Ctx, {scope}: i64) -> ::std::result::Result<{stem}ToggleResp, MeltDown> {{
    let session = ctx.require_session()?;
{session_lines_joined}
    let out = Crank::none().run(|| crate::routines::generated::{table}_toggle::run(ctx, {routine_args_joined})).await?;
    ctx.publish("{table}:list", &out);
    ctx.publish(&format!("{table}:row:{{}}", {scope}), &out);
    ctx.publish(&format!("{table}:{scope}:{{}}", {scope}), &out);
    Ok(out)
}}
"#
    )
}

fn render_http(table: &str, stem: &str, scope: &str) -> String {
    format!(
        r#"use axum::extract::Path;
use axum::routing::post;
use axum::{{Extension, Json, Router}};

use crate::flows::generated::{table}_toggle as flow;
use crate::meltdown::MeltDown;
use crate::structs::generated::{table}_toggle::{stem}ToggleResp;
use crate::Ctx;

pub async fn handle(
    Extension(ctx): Extension<Ctx>,
    Path({scope}): Path<i64>,
) -> ::std::result::Result<Json<{stem}ToggleResp>, MeltDown> {{
    let resp = flow::run(&ctx, {scope}).await?;
    Ok(Json(resp))
}}

pub fn router() -> Router<Ctx> {{
    Router::new().route("/:{scope}", post(handle))
}}
"#
    )
}

fn render_leptos_data(table: &str, stem: &str, scope: &str) -> String {
    format!(
        r#"use crate::meltdown::MeltDown;
use crate::structs::generated::{table}_toggle::{stem}ToggleResp;

pub async fn run({scope}: i64) -> ::std::result::Result<{stem}ToggleResp, MeltDown> {{
    #[cfg(not(target_arch = "wasm32"))]
    {{
        let ctx = ::leptos::prelude::expect_context::<crate::ctx::Ctx>();
        crate::flows::generated::{table}_toggle::run(&ctx, {scope}).await
    }}
    #[cfg(target_arch = "wasm32")]
    {{
        let path = format!("/api/{table}/toggle/{{}}", {scope});
        let body: ::serde_json::Value = ::serde_json::json!({{}});
        crate::transport::leptos::api_client::post_json(&path, &body).await
    }}
}}
"#
    )
}

fn write(target: PathBuf, body: String, report: &mut EmitReport) -> BlastResult<()> {
    let parent = match target.parent() {
        Some(p) => p,
        None => return Ok(()), // allow: target without parent — bail no-op
    };
    fs::create_dir_all(parent)?;
    fs::write(&target, body)?;
    report.written.push(target);
    Ok(())
}

fn update_barrel(barrel_path: PathBuf, table: &str, _report: &mut EmitReport) -> BlastResult<()> {
    let line = format!("pub mod {table}_toggle;\n");
    let existing = match fs::read_to_string(&barrel_path) {
        Ok(s) => s,
        Err(_io) => String::new(), // allow: barrel not yet created; treat as empty
    };
    if existing.contains(line.trim_end()) {
        return Ok(());
    }
    let mut updated = existing.clone();
    if !updated.ends_with('\n') && !updated.is_empty() {
        updated.push('\n');
    }
    updated.push_str(&line);
    fs::write(&barrel_path, &updated)?;
    Ok(())
}

fn update_http_router(project_root: &Path, table: &str, _report: &mut EmitReport) -> BlastResult<()> {
    let path = project_root.join("src/transport/http/generated/router.rs");
    let body = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_io) => return Ok(()), // allow: router not yet emitted; main http pass owns it
    };
    let nest_line = format!(
        "    router = router.nest(\"/{table}/toggle\", crate::transport::http::generated::{table}_toggle::router());\n"
    );
    if body.contains(nest_line.trim()) {
        return Ok(());
    }
    let needle = "    router\n}";
    let updated = body.replace(needle, &format!("{nest_line}    router\n}}"));
    fs::write(&path, &updated)?;
    Ok(())
}
