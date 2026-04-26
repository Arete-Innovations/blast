//! `blast gen all` — default codegen pipeline.
//!
//! Pipeline order:
//!     schema → structs → models → flows → http_routes
//!            → frontend_types
//!            → theme → icons → env_example → governor_plugin
//!
//! FE composables / api clients / Vue components / CRUD pages are opt-in
//! via `blast gen pages [<resource>]`, `blast gen api [<resource>]`,
//! `blast gen types [<resource>]`. Default keeps FE side to types only.

use std::path::PathBuf;

use crate::codegen;
use crate::configs::Config;
use crate::database;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::state;

#[derive(Debug, Clone)]
pub struct Args {
    pub project_root: PathBuf,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Outcome {
    pub steps_run: usize,
    pub files_written: usize,
    pub files_skipped: usize,
}

const STEP_SCHEMA: &str = "schema generation";
const STEP_STRUCTS: &str = "structs generation";
const STEP_MODELS: &str = "models generation";
const STEP_FLOWS: &str = "flows generation";
const STEP_HTTP_ROUTES: &str = "http routes generation";
const STEP_FRONTEND_TYPES: &str = "frontend types generation";
const STEP_THEME: &str = "theme codegen";
const STEP_ICONS: &str = "icons codegen";
const STEP_ENV_EXAMPLE: &str = ".env.example generation";
const STEP_GOVERNOR_PLUGIN: &str = "governor plugin emission";

pub fn run(
    args: Args,
    config: &mut Config,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<Outcome> {
    sink.info(format!(
        "blast gen all: pipeline starting at {}",
        args.project_root.display()
    ));

    let resource_count = match preflight_resources(&args.project_root) {
        Ok(n) => n,
        Err(err) => {
            sink.warn(format!(
                "state directory unreadable ({}); proceeding with zero resources",
                err
            ));
            0
        }
    };
    sink.info(format!(
        "discovered {} resource state file(s)",
        resource_count
    ));

    let mut outcome = Outcome::default();

    run_schema_step(sink, progress, &mut outcome)?;
    run_structs_step(&args.project_root, sink, progress, &mut outcome)?;
    run_models_step(&args.project_root, config, sink, progress, &mut outcome)?;
    run_flows_step(&args.project_root, sink, progress, &mut outcome)?;
    run_http_routes_step(&args.project_root, sink, progress, &mut outcome)?;
    run_frontend_types_step(&args.project_root, sink, progress, &mut outcome)?;
    run_theme_step(&args.project_root, sink, progress, &mut outcome)?;
    run_icons_step(&args.project_root, sink, progress, &mut outcome)?;
    run_env_example_step(&args.project_root, sink, progress, &mut outcome)?;
    run_governor_plugin_step(&args.project_root, sink, progress, &mut outcome)?;

    sink.success(format!(
        "blast gen all: {} step(s) ran, {} file(s) written, {} file(s) skipped",
        outcome.steps_run, outcome.files_written, outcome.files_skipped
    ));

    Ok(outcome)
}

fn preflight_resources(project_root: &PathBuf) -> BlastResult<usize> {
    let state_dir = project_root.join("storage").join("blast").join("state");
    if !state_dir.is_dir() {
        return Ok(0);
    }
    let names = state::list_resources(&state_dir)?;
    Ok(names.len())
}

fn run_schema_step(
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    progress.step_start(STEP_SCHEMA);
    let ok = database::generate_schema();
    if !ok {
        let reason = "diesel print-schema failed; see logs";
        progress.step_fail(STEP_SCHEMA, reason);
        sink.error(format!("{}: {}", STEP_SCHEMA, reason));
        return Err(BlastError::Subprocess {
            cmd: "diesel print-schema".to_string(),
            detail: reason.to_string(),
        });
    }
    progress.step_done(STEP_SCHEMA);
    outcome.steps_run += 1;
    Ok(())
}

fn run_structs_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    match codegen::structs::run(project_root, sink, progress) {
        Ok(report) => {
            for path in &report.written {
                sink.info(format!("wrote {}", path.display()));
            }
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_STRUCTS, &reason);
            sink.error(format!("{}: {}", STEP_STRUCTS, reason));
            Err(err)
        }
    }
}

fn run_models_step(
    project_root: &PathBuf,
    _config: &mut Config,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    match codegen::models::run(project_root, sink, progress) {
        Ok(report) => {
            for path in &report.written {
                sink.info(format!("wrote {}", path.display()));
            }
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_MODELS, &reason);
            sink.error(format!("{}: {}", STEP_MODELS, reason));
            Err(err)
        }
    }
}

fn run_flows_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    match codegen::flows::run(project_root, sink, progress) {
        Ok(report) => {
            for path in &report.written {
                sink.info(format!("wrote {}", path.display()));
            }
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_FLOWS, &reason);
            sink.error(format!("{}: {}", STEP_FLOWS, reason));
            Err(err)
        }
    }
}

fn run_http_routes_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    match codegen::http_routes::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            sink.info(format!(
                "{}: {} written, {} skipped",
                STEP_HTTP_ROUTES,
                report.written.len(),
                report.skipped.len()
            ));
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_HTTP_ROUTES, err));
            Err(err)
        }
    }
}

fn run_frontend_types_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    match codegen::frontend_types::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            sink.info(format!(
                "{}: {} written, {} skipped",
                STEP_FRONTEND_TYPES,
                report.written.len(),
                report.skipped.len()
            ));
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_FRONTEND_TYPES, err));
            Err(err)
        }
    }
}

fn run_env_example_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    progress.step_start(STEP_ENV_EXAMPLE);
    match codegen::env_example::run(project_root, sink, progress) {
        Ok(report) => {
            match report.written {
                Some(path) => {
                    sink.info(format!("emitted {}", path.display()));
                    outcome.files_written += 1;
                }
                None => {}
            }
            progress.step_done(STEP_ENV_EXAMPLE);
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_ENV_EXAMPLE, &reason);
            sink.error(format!("{}: {}", STEP_ENV_EXAMPLE, reason));
            Err(err)
        }
    }
}

fn run_theme_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    match codegen::theme::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_THEME, err));
            Err(err)
        }
    }
}

fn run_icons_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    match codegen::icons::run(project_root, sink, progress) {
        Ok(report) => {
            if report.written.is_some() {
                outcome.files_written += 1;
            }
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_ICONS, err));
            Err(err)
        }
    }
}

fn run_governor_plugin_step(
    project_root: &PathBuf,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    progress.step_start(STEP_GOVERNOR_PLUGIN);
    match codegen::governor_plugin::run(project_root) {
        Ok(emitted) => {
            for path in &emitted {
                sink.info(format!("emitted {}", path.display()));
            }
            outcome.files_written += emitted.len();
            progress.step_done(STEP_GOVERNOR_PLUGIN);
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_GOVERNOR_PLUGIN, &reason);
            sink.error(format!("{}: {}", STEP_GOVERNOR_PLUGIN, reason));
            Err(err)
        }
    }
}
