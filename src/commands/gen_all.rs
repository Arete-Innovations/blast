//! `blast gen all` — sequence every codegen target, surface step-by-step status.
//!
//! Pipeline order (per SPEC_BLAST_COMMANDS):
//!     schema → structs → models → flows → frontend
//!            → env-example → governor-plugin → test scaffolds
//!
//! Steps that have no underlying generator yet (`env-example`) emit a
//! `sink.warn` and are skipped without aborting the pipeline. Every other step
//! is run via its existing handler; on failure the pipeline aborts and the
//! error propagates to the caller. No retries — that is `blast init`'s job.

use std::path::PathBuf;

use crate::codegen;
use crate::configs::Config;
use crate::database;
use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::models;
use crate::state;
use crate::structs;

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
const STEP_FRONTEND: &str = "frontend generation";
const STEP_WS_TOPICS: &str = "ws topics generation";
const STEP_ENV_EXAMPLE: &str = ".env.example generation";
const STEP_GOVERNOR_PLUGIN: &str = "governor plugin emission";
const STEP_TEST_SCAFFOLDS: &str = "test scaffold emission";

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
    run_structs_step(config, sink, progress, &mut outcome)?;
    run_models_step(config, sink, progress, &mut outcome)?;
    run_flows_step(&args.project_root, sink, progress, &mut outcome)?;
    run_http_routes_step(&args.project_root, resource_count, sink, progress, &mut outcome)?;
    run_frontend_step(&args.project_root, resource_count, sink, progress, &mut outcome)?;
    run_ws_topics_step(&args.project_root, resource_count, sink, progress, &mut outcome)?;
    run_env_example_step(&args.project_root, sink, progress, &mut outcome)?;
    run_governor_plugin_step(&args.project_root, sink, progress, &mut outcome)?;
    run_test_scaffolds_step(&args.project_root, resource_count, sink, progress, &mut outcome)?;

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
    config: &mut Config,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    progress.step_start(STEP_STRUCTS);
    let ok = structs::generate(config);
    if !ok {
        let reason = "structs generator reported failure";
        progress.step_fail(STEP_STRUCTS, reason);
        sink.warn(format!(
            "{}: {} (continuing — may be normal for empty schemas)",
            STEP_STRUCTS, reason
        ));
    } else {
        progress.step_done(STEP_STRUCTS);
    }
    outcome.steps_run += 1;
    Ok(())
}

fn run_models_step(
    config: &mut Config,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    progress.step_start(STEP_MODELS);
    let ok = models::generate(config);
    if !ok {
        let reason = "models generator reported failure";
        progress.step_fail(STEP_MODELS, reason);
        sink.warn(format!(
            "{}: {} (continuing — may be normal for empty schemas)",
            STEP_MODELS, reason
        ));
    } else {
        progress.step_done(STEP_MODELS);
    }
    outcome.steps_run += 1;
    Ok(())
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
    resource_count: usize,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    if resource_count == 0 {
        progress.step_start(STEP_HTTP_ROUTES);
        sink.info(format!(
            "{}: no resources declared; skipping",
            STEP_HTTP_ROUTES
        ));
        progress.step_done(STEP_HTTP_ROUTES);
        outcome.steps_run += 1;
        return Ok(());
    }
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

fn run_frontend_step(
    project_root: &PathBuf,
    resource_count: usize,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    progress.step_start(STEP_FRONTEND);
    if resource_count == 0 {
        sink.info(format!(
            "{}: no resources declared; skipping",
            STEP_FRONTEND
        ));
        progress.step_done(STEP_FRONTEND);
        outcome.steps_run += 1;
        return Ok(());
    }
    match codegen::run_frontend(project_root) {
        Ok(()) => {
            progress.step_done(STEP_FRONTEND);
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_FRONTEND, &reason);
            sink.error(format!("{}: {}", STEP_FRONTEND, reason));
            Err(err)
        }
    }
}

fn run_ws_topics_step(
    project_root: &PathBuf,
    resource_count: usize,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    if resource_count == 0 {
        progress.step_start(STEP_WS_TOPICS);
        sink.info(format!(
            "{}: no resources declared; skipping",
            STEP_WS_TOPICS
        ));
        progress.step_done(STEP_WS_TOPICS);
        outcome.steps_run += 1;
        return Ok(());
    }
    match codegen::ws_topics::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            sink.info(format!(
                "{}: {} written, {} skipped",
                STEP_WS_TOPICS,
                report.written.len(),
                report.skipped.len()
            ));
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_WS_TOPICS, err));
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

fn run_test_scaffolds_step(
    project_root: &PathBuf,
    resource_count: usize,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
    outcome: &mut Outcome,
) -> BlastResult<()> {
    progress.step_start(STEP_TEST_SCAFFOLDS);
    if resource_count == 0 {
        sink.info(format!(
            "{}: no resources declared; skipping",
            STEP_TEST_SCAFFOLDS
        ));
        progress.step_done(STEP_TEST_SCAFFOLDS);
        outcome.steps_run += 1;
        return Ok(());
    }
    let filter = codegen::test_scaffold::Filter::All;
    match codegen::test_scaffold::run(project_root, &filter) {
        Ok(report) => {
            for path in &report.written {
                sink.info(format!("wrote {}", path.display()));
            }
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            sink.info(format!(
                "{}: {} written, {} skipped",
                STEP_TEST_SCAFFOLDS,
                report.written.len(),
                report.skipped.len()
            ));
            progress.step_done(STEP_TEST_SCAFFOLDS);
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            let reason = err.to_string();
            progress.step_fail(STEP_TEST_SCAFFOLDS, &reason);
            sink.error(format!("{}: {}", STEP_TEST_SCAFFOLDS, reason));
            Err(err)
        }
    }
}
