//! Pipeline driver for the FE router codegen pass.
//!
//! Inputs:
//!  - `storage/blast/state/app.ron`     — `Nav(NavConfig)` + `Pages([Page])`
//!  - `storage/blast/state/resources/<name>.ron` — verbs per resource
//!
//! Outputs:
//!  - `frontend/src/generated/router/routes.ts`
//!  - `frontend/src/generated/router/route-names.ts`
//!  - `frontend/src/generated/router/install-router-guards.ts`
//!  - `frontend/src/generated/nav/menu.ts`
//!
//! Drift impossible by construction: every `Entry.route` in NavConfig is
//! validated against the resolved CRUD-plus-pages route set; dangling
//! references abort codegen with a `BlastError::Invalid` carrying the
//! offending section/route names. Per-entry roles must be a subset of the
//! referenced route's effective auth requirement.

use std::fs;
use std::path::{Path, PathBuf};

use crate::codegen::header;
use crate::codegen::ir_loader;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state;
use crate::state::{AppPolicySection, AppState, NavConfig, Page};

use super::{guards, menu, resolve, route_names, routes, validate};

#[derive(Debug, Default)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

const STEP_LABEL: &str = "router codegen";

const ROUTES_RELATIVE: &str = "frontend/src/generated/router/routes.ts";
const ROUTE_NAMES_RELATIVE: &str = "frontend/src/generated/router/route-names.ts";
const GUARDS_RELATIVE: &str = "frontend/src/generated/router/install-router-guards.ts";
const MENU_RELATIVE: &str = "frontend/src/generated/nav/menu.ts";

pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<EmitReport> {
    progress.step_start(STEP_LABEL);
    let result = run_inner(project_root, sink);
    match &result {
        Ok(_report) => progress.step_done(STEP_LABEL),
        Err(err) => progress.step_fail(STEP_LABEL, err.to_string().as_str()),
    }
    result
}

fn run_inner(project_root: &Path, sink: &mut dyn Sink) -> BlastResult<EmitReport> {
    let state_dir = project_root.join("storage").join("blast").join("state");
    let app_state = load_app_or_default(&state_dir)?;
    let resources = ir_loader::load_resource_states(project_root)?;

    let nav = extract_nav(&app_state);
    let pages = extract_pages(&app_state);

    let resolved = resolve::resolve_all(&resources, &pages);
    validate::validate_nav_against_routes(nav.as_ref(), &resolved)?;

    let mut report = EmitReport::default();
    let app_marker = header::marker_for_app(project_root)?;

    write_emitted(
        project_root,
        ROUTES_RELATIVE,
        &format!("{}{}", app_marker, routes::render(&resolved)),
        &mut report,
        sink,
    )?;
    write_emitted(
        project_root,
        ROUTE_NAMES_RELATIVE,
        &format!("{}{}", app_marker, route_names::render(&resolved)),
        &mut report,
        sink,
    )?;
    write_emitted(
        project_root,
        GUARDS_RELATIVE,
        &format!("{}{}", app_marker, guards::render()),
        &mut report,
        sink,
    )?;
    write_emitted(
        project_root,
        MENU_RELATIVE,
        &format!("{}{}", app_marker, menu::render(nav.as_ref(), &resolved)),
        &mut report,
        sink,
    )?;

    Ok(report)
}

fn load_app_or_default(state_dir: &Path) -> BlastResult<AppState> {
    match state::load_app(state_dir) {
        Ok(s) => Ok(s),
        Err(BlastError::Io(io_err)) => match io_err.kind() {
            std::io::ErrorKind::NotFound => Ok(AppState::default()),
            other_kind => Err(BlastError::Io(std::io::Error::new(other_kind, io_err))),
        },
        Err(other) => Err(other),
    }
}

fn extract_nav(app: &AppState) -> Option<NavConfig> {
    for (_k, section) in &app.sections {
        match section {
            AppPolicySection::Nav(nav) => return Some(nav.clone()),
            AppPolicySection::FeLint(_)
            | AppPolicySection::Admin(_)
            | AppPolicySection::Fuses(_)
            | AppPolicySection::Services(_)
            | AppPolicySection::EnvSpec(_)
            | AppPolicySection::Defaults(_)
            | AppPolicySection::Pages(_) => continue,
        }
    }
    None
}

fn extract_pages(app: &AppState) -> Vec<Page> {
    for (_k, section) in &app.sections {
        match section {
            AppPolicySection::Pages(pages) => return pages.clone(),
            AppPolicySection::FeLint(_)
            | AppPolicySection::Admin(_)
            | AppPolicySection::Fuses(_)
            | AppPolicySection::Services(_)
            | AppPolicySection::EnvSpec(_)
            | AppPolicySection::Defaults(_)
            | AppPolicySection::Nav(_) => continue,
        }
    }
    Vec::new()
}

fn write_emitted(
    project_root: &Path,
    relative: &str,
    body: &str,
    report: &mut EmitReport,
    sink: &mut dyn Sink,
) -> BlastResult<()> {
    let target = project_root.join(relative);
    let parent = match target.parent() {
        Some(p) => p,
        None => {
            return Err(BlastError::Invalid(format!(
                "router codegen target has no parent: {}",
                target.display()
            )))
        }
    };
    fs::create_dir_all(parent)?;
    fs::write(&target, body)?;
    report.written.push(target.clone());
    sink.info(format!("emitted {}", target.display()));
    Ok(())
}
