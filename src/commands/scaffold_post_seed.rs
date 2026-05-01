//! Post-seed pipeline injected into `blast new` / `blast init`.
//!
//! Pipeline order — runs after the file-writing scaffold and before
//! the dashboard exec:
//!   1. diesel migration run    — applies the seed migrations against the bootstrapped DB.
//!   2. cargo build debug       — pre-compiles the user app (skipped via `--no-warmup`).
//!
//! NO codegen step. Templates ship a working hand-written app (auth +
//! sessions). Codegen is opt-in via `blast gen <subcmd>` once the user
//! adds their own resources.
//!
//! All sub-steps run with cwd swapped to project_root so diesel CLI / cargo
//! pick up the right `.env` and `Cargo.toml`. cwd restored on every exit.

use std::path::Path;

use crate::{
    error::{BlastError, BlastResult},
    io::traits::{Progress, ProgressExt, Sink, SinkExt},
};

pub fn run(project_root: &Path, no_warmup: bool, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<usize> {
    let original_cwd = std::env::current_dir()?;
    std::env::set_current_dir(project_root)?;

    let result = run_inner(project_root, no_warmup, sink, progress);

    if let Err(restore_err) = std::env::set_current_dir(&original_cwd) {
        sink.warn(format!("failed to restore cwd to {}: {}", original_cwd.display(), restore_err));
    }

    result
}

fn run_inner(project_root: &Path, no_warmup: bool, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<usize> {
    progress.step_start("run pending migrations");
    if !crate::database::migrate() {
        progress.step_fail("run pending migrations", "diesel migration run failed (see logs)");
        return Err(BlastError::Subprocess {
            cmd: "diesel migration run".to_string(),
            detail: "migration step failed during scaffold".to_string(),
        });
    }
    progress.step_done("run pending migrations");

    if no_warmup {
        sink.info("--no-warmup set; skipping cargo build pre-compile".to_string());
        return Ok(0);
    }

    progress.step_start("pre-compile backend (cargo build)");
    sink.info("pre-compiling backend with `cargo build` — first run can take 3-5 min on a cold host".to_string());
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
        let tail: String = stderr.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        progress.step_fail("pre-compile backend (cargo build)", "cargo build failed during scaffold");
        sink.error(format!("pre-compile backend failed; project dir kept for inspection. Last cargo stderr lines:\n{}", tail));
        return Err(BlastError::Subprocess {
            cmd: "cargo build".to_string(),
            detail: "pre-compile failed during scaffold".to_string(),
        });
    }
    progress.step_done("pre-compile backend (cargo build)");

    Ok(0)
}
