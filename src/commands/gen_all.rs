//! `blast gen all` — default codegen pipeline.
//!
//! Pipeline order (post-phase-2 leptos-shaped):
//!     schema → enums → structs → validators → models → routines → flows
//!            → http_routes (REST /api/*) → leptos_forms → leptos_pages
//!            → leptos_tables → app_routes → env_example
//!
//! Leptos forms run before leptos pages because pages reference form
//! components (e.g. `<UserCreateForm/>`) emitted by the forms pass. Both
//! passes touch `transport/leptos/data/generated/` for stub helpers — the
//! later (pages) emitter is a strict superset, so its writes overwrite
//! the earlier (forms) writes when both qualify.

use std::path::{Path, PathBuf};

use crate::{
    codegen,
    codegen::ir_loader,
    configs::Config,
    database,
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
    state,
    state::{GenLevel, ResourceState},
};

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
const STEP_ENUMS: &str = "enums generation";
const STEP_STRUCTS: &str = "structs generation";
const STEP_MODELS: &str = "models generation";
const STEP_ROUTINES: &str = "routines generation";
const STEP_FLOWS: &str = "flows generation";
const STEP_HTTP_ROUTES: &str = "http routes generation";
const STEP_VALIDATORS: &str = "validators generation";
const STEP_LEPTOS_PAGES: &str = "leptos pages generation";
const STEP_LEPTOS_FORMS: &str = "leptos forms generation";
const STEP_LEPTOS_TABLES: &str = "leptos tables generation";
const STEP_APP_ROUTES: &str = "leptos app routes generation";
const STEP_ENV_EXAMPLE: &str = ".env.example generation";

pub fn run(args: Args, config: &mut Config, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<Outcome> {
    sink.info(format!("blast gen all: pipeline starting at {}", args.project_root.display()));

    let resource_count = match preflight_resources(&args.project_root) {
        Ok(n) => n,
        Err(err) => {
            sink.warn(format!("state directory unreadable ({}); proceeding with zero resources", err));
            0
        }
    };
    sink.info(format!("discovered {} resource state file(s)", resource_count));

    let mut outcome = Outcome::default();

    run_schema_step(sink, progress, &mut outcome)?;
    run_enums_step(&args.project_root, sink, progress, &mut outcome)?;
    run_structs_step(&args.project_root, sink, progress, &mut outcome)?;
    run_models_step(&args.project_root, config, sink, progress, &mut outcome)?;
    run_routines_step(&args.project_root, sink, progress, &mut outcome)?;
    run_flows_step(&args.project_root, sink, progress, &mut outcome)?;
    run_http_routes_step(&args.project_root, sink, progress, &mut outcome)?;
    run_validators_step(&args.project_root, sink, progress, &mut outcome)?;
    run_leptos_forms_step(&args.project_root, sink, progress, &mut outcome)?;
    run_leptos_pages_step(&args.project_root, sink, progress, &mut outcome)?;
    run_leptos_tables_step(&args.project_root, sink, progress, &mut outcome)?;
    run_app_routes_step(&args.project_root, sink, progress, &mut outcome)?;
    run_env_example_step(&args.project_root, sink, progress, &mut outcome)?;

    warn_on_orphan_generated(&args.project_root, sink);

    sink.success(format!(
        "blast gen all: {} step(s) ran, {} file(s) written, {} file(s) skipped",
        outcome.steps_run, outcome.files_written, outcome.files_skipped
    ));

    Ok(outcome)
}

/// Inspect each resource's `gen_level` and warn the user when generated files
/// exist on disk for a level higher than the current cut-off. Blast does NOT
/// auto-delete — the user must clean up manually so accidental level drops
/// can't silently destroy work.
fn warn_on_orphan_generated(project_root: &Path, sink: &mut dyn Sink) {
    let resources = match ir_loader::load_resource_states(project_root) {
        Ok(rs) => rs,
        Err(_err) => return,
    };

    for resource in &resources {
        warn_resource_orphans(project_root, resource, sink);
    }
}

fn warn_resource_orphans(project_root: &Path, resource: &ResourceState, sink: &mut dyn Sink) {
    let table = resource.name.as_str();
    let level = resource.gen_level;

    let checks: &[(GenLevel, PathBuf, &'static str)] = &[
        (GenLevel::Model, project_root.join("src").join("models").join("generated").join(format!("{table}.rs")), "models/generated"),
        (GenLevel::Route, project_root.join("src").join("routines").join("generated").join(table), "routines/generated"),
        (GenLevel::Route, project_root.join("src").join("flows").join("generated").join(table), "flows/generated"),
        (
            GenLevel::Route,
            project_root.join("src").join("transport").join("http").join("generated").join(format!("{table}.rs")),
            "transport/http/generated",
        ),
        (
            GenLevel::Types,
            project_root.join("src").join("structs").join("generated").join("validators").join(format!("{table}.rs")),
            "structs/generated/validators",
        ),
    ];

    for (required_level, path, label) in checks {
        if level >= *required_level {
            continue;
        }
        if !path.exists() {
            continue;
        }
        sink.warn(format!(
            "orphan codegen: resource '{}' is at gen_level {:?} (below {:?}); stale {} at {} will not be regenerated — delete manually",
            table,
            level,
            required_level,
            label,
            path.display(),
        ));
    }
}

fn preflight_resources(project_root: &PathBuf) -> BlastResult<usize> {
    let state_dir = project_root.join("storage").join("blast").join("state");
    if !state_dir.is_dir() {
        return Ok(0);
    }
    let names = state::list_resources(&state_dir)?;
    Ok(names.len())
}

fn run_schema_step(sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
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

fn run_enums_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::enums::run(project_root, sink, progress) {
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
            progress.step_fail(STEP_ENUMS, &reason);
            sink.error(format!("{}: {}", STEP_ENUMS, reason));
            Err(err)
        }
    }
}

fn run_structs_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
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

fn run_models_step(project_root: &PathBuf, _config: &mut Config, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
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

fn run_routines_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::routines::run(project_root, sink, progress) {
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
            progress.step_fail(STEP_ROUTINES, &reason);
            sink.error(format!("{}: {}", STEP_ROUTINES, reason));
            Err(err)
        }
    }
}

fn run_flows_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
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

fn run_http_routes_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::http_routes::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            sink.info(format!("{}: {} written, {} skipped", STEP_HTTP_ROUTES, report.written.len(), report.skipped.len()));
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_HTTP_ROUTES, err));
            Err(err)
        }
    }
}

fn run_validators_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::validators::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            sink.info(format!("{}: {} written, {} skipped", STEP_VALIDATORS, report.written.len(), report.skipped.len()));
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_VALIDATORS, err));
            Err(err)
        }
    }
}

fn run_leptos_pages_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::leptos_pages::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_LEPTOS_PAGES, err));
            Err(err)
        }
    }
}

fn run_leptos_forms_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::leptos_forms::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_LEPTOS_FORMS, err));
            Err(err)
        }
    }
}

fn run_leptos_tables_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::leptos_tables::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_LEPTOS_TABLES, err));
            Err(err)
        }
    }
}

fn run_app_routes_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
    match codegen::app_routes::run(project_root, sink, progress) {
        Ok(report) => {
            outcome.files_written += report.written.len();
            outcome.files_skipped += report.skipped.len();
            outcome.steps_run += 1;
            Ok(())
        }
        Err(err) => {
            sink.error(format!("{}: {}", STEP_APP_ROUTES, err));
            Err(err)
        }
    }
}

fn run_env_example_step(project_root: &PathBuf, sink: &mut dyn Sink, progress: &mut dyn Progress, outcome: &mut Outcome) -> BlastResult<()> {
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

