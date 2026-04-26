//! Phase 12 post-seed pipeline injected into `blast new` / `blast init`.
//!
//! Pipeline order — runs after the file-writing scaffold and before npm
//! install / dashboard exec:
//!   1. diesel migration run    — populates the bootstrapped DB so
//!                                diesel print-schema sees tables.
//!   2. blast gen all           — schema + structs + models + flows +
//!                                http routes + frontend + ws topics +
//!                                vue components + crud pages + router +
//!                                theme + icons + env example + governor
//!                                plugin + test scaffolds.
//!   3. cargo build debug       — pre-compiles the user app so the first
//!                                blast run / cargo run is instant.
//!
//! Lives in the bin tree, not in the lib-side scaffold module, because
//! it depends on bin-private modules — gen_all, database, configs. The
//! scaffold module injects this hook via the post_seed field on
//! NewOptions so lib-side scaffold tests stay DB-free.
//!
//! All sub-steps run with cwd swapped to project_root. Reasons —
//!   - diesel CLI looks up diesel.toml and .env relative to cwd.
//!   - generate_schema writes to src/database/schema.rs relative to cwd.
//!   - cargo build needs to find Cargo.toml in cwd.
//!
//! The cwd is restored on every exit path so subsequent tests and TUI
//! launches don't see a leaked chdir.

use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use std::path::Path;

/// Entry point matching the PostSeedHook signature from the scaffold module.
pub fn run(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<usize> {
    let original_cwd = std::env::current_dir()?;
    std::env::set_current_dir(project_root)?;

    let result = run_inner(project_root, sink, progress);

    // Restore cwd unconditionally — never leak a chdir.
    if let Err(restore_err) = std::env::set_current_dir(&original_cwd) {
        sink.warn(format!(
            "failed to restore cwd to {}: {}",
            original_cwd.display(),
            restore_err
        ));
    }

    result
}

fn run_inner(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<usize> {
    // Run pending migrations against the bootstrapped DB so
    // `diesel print-schema` (called inside gen_all) sees real tables.
    progress.step_start("run pending migrations");
    if !crate::database::migrate() {
        progress.step_fail(
            "run pending migrations",
            "diesel migration run failed (see logs)",
        );
        return Err(BlastError::Subprocess {
            cmd: "diesel migration run".to_string(),
            detail: "migration step failed during scaffold".to_string(),
        });
    }
    progress.step_done("run pending migrations");

    // Build a Config the gen_all pipeline can consume. Reads project_name
    // from the freshly-vendored Cargo.toml.
    let mut config = crate::configs::build_config(project_root)?;

    progress.step_start("blast gen all");
    let gen_args = crate::commands::gen_all::Args {
        project_root: project_root.to_path_buf(),
    };
    let gen_outcome = crate::commands::gen_all::run(gen_args, &mut config, sink, progress)?;
    progress.step_done("blast gen all");
    sink.info(format!(
        "blast gen all wrote {} file(s) across {} step(s) (skipped {})",
        gen_outcome.files_written, gen_outcome.steps_run, gen_outcome.files_skipped
    ));

    // Pre-compile the user app so the first `blast run` / `cargo run`
    // doesn't pay the cold-build tax. Debug profile — typically 3-5 min
    // on a fresh host with no sccache. We tell the user up-front so the
    // pause looks intentional, not stuck.
    progress.step_start("pre-compile backend (cargo build)");
    sink.info(
        "pre-compiling backend with `cargo build` — first run can take 3-5 min on a cold host"
            .to_string(),
    );
    let cargo_status = std::process::Command::new("cargo")
        .args(["build"])
        .current_dir(project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| BlastError::Subprocess {
            cmd: "cargo build".to_string(),
            detail: format!("spawn failed: {}", e),
        })?;
    if !cargo_status.status.success() {
        let stderr = String::from_utf8_lossy(&cargo_status.stderr);
        let tail: String = stderr
            .lines()
            .rev()
            .take(40)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        progress.step_fail(
            "pre-compile backend (cargo build)",
            "cargo build failed during scaffold",
        );
        sink.error(format!(
            "pre-compile backend failed; project dir kept for inspection. Last cargo stderr lines:\n{}",
            tail
        ));
        return Err(BlastError::Subprocess {
            cmd: "cargo build".to_string(),
            detail: "pre-compile failed during scaffold".to_string(),
        });
    }
    progress.step_done("pre-compile backend (cargo build)");

    Ok(gen_outcome.files_written)
}
