//! Post-scaffold pipeline shared by `blast new` and `blast init`.
//!
//! Steps, in order:
//!   1. `npm install` in `<project_root>/frontend`
//!   2. `npm run build` in `<project_root>/frontend`
//!   3. `exec` into a fresh `blast` invocation (no args) at `<project_root>`,
//!      which lands the user in the zellij dashboard.
//!
//! Step 3 replaces the running blast process via the `exec` syscall —
//! when the dashboard exits, the user is dropped back at their original
//! shell cwd. Step 3 is skipped if the env var `BLAST_NO_TUI_FOR_TESTS=1`
//! is set; this is an internal-only escape hatch for verification scripts
//! (a real user wouldn't set it). Not a CLI flag by design.
//!
//! If `npm run build` fails, we log loudly but DO NOT remove the project
//! dir — the install already produced `node_modules` and the user can
//! inspect what broke. If `npm install` fails, ditto: we surface the
//! error but leave the partial state for inspection.

use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Env var that, when set to `1`, skips the auto-TUI exec step. Used by
/// the wave11 verification scripts so they don't block forever in a
/// dashboard. NOT a CLI flag — single-user dev machines do not get an
/// opt-out lever for the post-scaffold flow.
const SKIP_TUI_ENV: &str = "BLAST_NO_TUI_FOR_TESTS";

/// Run the full post-scaffold pipeline. Returns Ok on success; the
/// auto-TUI step replaces this process via exec, so on the happy path
/// the function never actually returns.
///
/// When `no_warmup` is true, ALL heavy steps are skipped: npm install,
/// npm run build, and the auto-TUI exec. The function returns Ok
/// immediately so the caller can print next-steps and exit.
pub fn run(
    project_root: &Path,
    no_warmup: bool,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<()> {
    let frontend_dir = project_root.join("frontend");
    if !frontend_dir.is_dir() {
        return Err(BlastError::Project(format!(
            "post-install: expected frontend dir at {} but none found",
            frontend_dir.display()
        )));
    }

    if no_warmup {
        sink.info("--no-warmup set; skipping npm install, npm run build, and TUI exec".to_string());
        return Ok(());
    }

    progress.step_start("frontend: npm install");
    match run_npm(&frontend_dir, &["install"], sink) {
        Ok(()) => {
            progress.step_done("frontend: npm install");
        }
        Err(e) => {
            progress.step_fail("frontend: npm install", format!("{}", e));
            return Err(e);
        }
    }

    progress.step_start("frontend: npm run build");
    match run_npm(&frontend_dir, &["run", "build"], sink) {
        Ok(()) => {
            progress.step_done("frontend: npm run build");
        }
        Err(e) => {
            // Per spec: log loudly, do NOT remove the project dir, surface
            // as error so caller can decide whether to abort the auto-TUI.
            progress.step_fail("frontend: npm run build", format!("{}", e));
            sink.error(format!(
                "npm run build failed; project dir kept at {} for inspection: {}",
                project_root.display(),
                e
            ));
            return Err(e);
        }
    }

    exec_into_tui(project_root, sink)
}

/// Spawn `npm <args>` in `cwd`, stream stdout+stderr line-by-line into
/// the sink (stdout as debug, stderr as warn), and return Err if the
/// process exits non-zero. Plain `npm` — no `--json` parsing — keeps the
/// implementation simple per spec ("prioritize working over fancy").
fn run_npm(cwd: &Path, args: &[&str], sink: &mut dyn Sink) -> BlastResult<()> {
    let display_cmd = format!("npm {}", args.join(" "));
    sink.info(format!("running `{}` in {}", display_cmd, cwd.display()));

    let mut child = Command::new("npm")
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            BlastError::Subprocess {
                cmd: display_cmd.clone(),
                detail: format!("spawn failed: {}", e),
            }
        })?;

    // Drain stdout. We take it before stderr so the read order is
    // deterministic; both are pipes so neither blocks the child.
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            return Err(BlastError::Subprocess {
                cmd: display_cmd,
                detail: "could not capture stdout".to_string(),
            });
        }
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            return Err(BlastError::Subprocess {
                cmd: display_cmd,
                detail: "could not capture stderr".to_string(),
            });
        }
    };

    // Read both streams in background threads to avoid pipe-fill deadlock
    // (npm install can emit megabytes of output before exiting).
    let stdout_lines = std::thread::spawn(move || drain_to_lines(stdout));
    let stderr_lines = std::thread::spawn(move || drain_to_lines(stderr));

    let status = child.wait().map_err(|e| BlastError::Subprocess {
        cmd: display_cmd.clone(),
        detail: format!("wait failed: {}", e),
    })?;

    let stdout_collected = match stdout_lines.join() {
        Ok(v) => v,
        Err(_panic) => Vec::new(), // allow: thread panic on drain shouldn't block error reporting
    };
    let stderr_collected = match stderr_lines.join() {
        Ok(v) => v,
        Err(_panic) => Vec::new(), // allow: thread panic on drain shouldn't block error reporting
    };

    for line in &stdout_collected {
        sink.debug(format!("npm: {}", line));
    }
    for line in &stderr_collected {
        // npm uses stderr for normal progress output too; surface as info
        // unless exit was non-zero (then they're echoed again as error
        // detail below).
        sink.info(format!("npm: {}", line));
    }

    if !status.success() {
        // Some npm-driven tools (vue-tsc, vite) emit diagnostics on
        // stdout, not stderr. If stderr is empty we fall back to
        // stdout's tail so the user gets something actionable.
        let tail_source = if stderr_collected.is_empty() {
            &stdout_collected
        } else {
            &stderr_collected
        };
        let tail: String = tail_source
            .iter()
            .rev()
            .take(30)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let code_str = match status.code() {
            Some(c) => c.to_string(),
            None => "?".to_string(),
        };
        return Err(BlastError::Subprocess {
            cmd: display_cmd,
            detail: format!("exit status {}: last output lines:\n{}", code_str, tail),
        });
    }

    Ok(())
}

fn drain_to_lines<R: std::io::Read>(reader: R) -> Vec<String> {
    let buf = BufReader::new(reader);
    let mut out = Vec::new();
    for line in buf.lines() {
        match line {
            Ok(s) => out.push(s),
            Err(_e) => break, // allow: read error ends drain; partial output already captured
        }
    }
    out
}

/// Replace the running blast process with a fresh `blast` invocation at
/// `project_root`. No args -> the `main.rs` default branch is `Command::Dashboard`
/// which launches the zellij TUI. The `exec` syscall means when the user
/// quits the dashboard, they land back in their original shell cwd.
///
/// Skipped (returns `Ok(())`) if `BLAST_NO_TUI_FOR_TESTS=1`.
fn exec_into_tui(project_root: &Path, sink: &mut dyn Sink) -> BlastResult<()> {
    let skip = match std::env::var(SKIP_TUI_ENV) {
        Ok(v) => v == "1",
        Err(_unset) => false, // allow: env var unset is the normal real-user case; not an error
    };
    if skip {
        sink.info(format!(
            "{}=1 set; skipping auto-TUI exec (project ready at {})",
            SKIP_TUI_ENV,
            project_root.display()
        ));
        return Ok(());
    }

    let self_exe: PathBuf = std::env::current_exe()?;
    sink.info(format!(
        "launching dashboard via {} in {}",
        self_exe.display(),
        project_root.display()
    ));

    use std::os::unix::process::CommandExt;
    let err = Command::new(&self_exe)
        .current_dir(project_root)
        .env_remove(SKIP_TUI_ENV) // belt-and-braces: the child shouldn't inherit a stale opt-out
        .exec();
    // exec() only returns on error.
    Err(BlastError::Project(format!(
        "failed to exec into TUI: {}",
        err
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::{NullProgress, NullSink};

    #[test]
    fn run_errors_when_frontend_dir_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        // No frontend/ subdir exists.
        let mut sink = NullSink;
        let mut progress = NullProgress;
        let err = run(dir.path(), false, &mut sink, &mut progress).expect_err("must fail");
        let msg = format!("{}", err);
        assert!(msg.contains("frontend dir"), "msg = {}", msg);
    }

    #[test]
    fn run_no_warmup_skips_pipeline() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("frontend")).expect("frontend dir");
        let mut sink = NullSink;
        let mut progress = NullProgress;
        run(dir.path(), true, &mut sink, &mut progress).expect("no-warmup short-circuits Ok");
    }

    #[test]
    fn exec_into_tui_skipped_when_env_set() {
        // SAFETY: tests in this crate run in a single process and may
        // race on env vars. The set/clear pair brackets just this test.
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var(SKIP_TUI_ENV, "1");
        let mut sink = NullSink;
        let result = exec_into_tui(dir.path(), &mut sink);
        std::env::remove_var(SKIP_TUI_ENV);
        result.expect("skip path returns Ok");
    }
}
