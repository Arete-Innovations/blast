//! Post-scaffold pipeline shared by `blast new` and `blast init`.
//!
//! With the FE rewritten to leptos, npm/vite are gone. The post-scaffold
//! step now just `exec`s into a fresh `blast` invocation (no args) at the
//! project root, landing the user in the zellij dashboard.
//!
//! Skipped if the env var `BLAST_NO_TUI_FOR_TESTS=1` is set; that's an
//! internal-only escape hatch for verification scripts.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    error::{BlastError, BlastResult},
    io::traits::{Progress, Sink, SinkExt},
};

const SKIP_TUI_ENV: &str = "BLAST_NO_TUI_FOR_TESTS";

pub fn run(project_root: &Path, _no_warmup: bool, sink: &mut dyn Sink, _progress: &mut dyn Progress) -> BlastResult<()> {
    exec_into_tui(project_root, sink)
}

fn exec_into_tui(project_root: &Path, sink: &mut dyn Sink) -> BlastResult<()> {
    let skip = match std::env::var(SKIP_TUI_ENV) {
        Ok(v) => v == "1",
        Err(_unset) => false, // allow: env var unset is the normal real-user case; not an error
    };
    if skip {
        sink.info(format!("{}=1 set; skipping auto-TUI exec (project ready at {})", SKIP_TUI_ENV, project_root.display()));
        return Ok(());
    }

    let self_exe: PathBuf = std::env::current_exe()?;
    sink.info(format!("launching dashboard via {} in {}", self_exe.display(), project_root.display()));

    use std::os::unix::process::CommandExt;
    let err = Command::new(&self_exe)
        .current_dir(project_root)
        .env_remove(SKIP_TUI_ENV)
        .exec();
    Err(BlastError::Project(format!("failed to exec into TUI: {}", err)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::{NullProgress, NullSink};

    #[test]
    fn exec_into_tui_skipped_when_env_set() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var(SKIP_TUI_ENV, "1");
        let mut sink = NullSink;
        let result = exec_into_tui(dir.path(), &mut sink);
        std::env::remove_var(SKIP_TUI_ENV);
        result.expect("skip path returns Ok");
    }

    #[test]
    fn run_no_warmup_skipped_via_env_returns_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var(SKIP_TUI_ENV, "1");
        let mut sink = NullSink;
        let mut progress = NullProgress;
        let result = run(dir.path(), true, &mut sink, &mut progress);
        std::env::remove_var(SKIP_TUI_ENV);
        result.expect("no-warmup with skip env returns Ok");
    }
}
