use std::path::Path;
use std::process::Command;

use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::project::scaffold::Source;

const VENDORED_PATHS: &[&str] = &[
    "src/services/vendored",
    "src/structs/vendored",
    "src/views/vendored",
    "catalyst-derive",
    "build.rs",
];

pub fn run_sync(dev: bool, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<()> {
    let project_root = std::env::current_dir()?;
    if !project_root.join("Cargo.toml").is_file() {
        return Err(BlastError::Project(format!("no Cargo.toml in {} — run from project root", project_root.display())));
    }

    let source = match dev {
        true => Source::dev_from_env()?,
        false => Source::git_default(),
    };

    if let Source::LocalCopy { path, .. } = &source {
        if path == &project_root {
            return Err(BlastError::Project(format!(
                "blast sync --dev: BLAST_CATALYST_DEV_PATH ({}) is the project root — refusing (would truncate files via self-overwrite)",
                path.display()
            )));
        }
    }

    let temp = tempfile::tempdir()?;
    let staging: std::path::PathBuf = match &source {
        Source::LocalCopy { path, .. } => {
            sink.info(format!("blast sync: reading working tree at {} (dev mode — no git clone)", path.display()));
            path.clone()
        }
        Source::Git { url, .. } => {
            let target = temp.path().join("catalyst");
            sink.info(format!("blast sync: cloning catalyst from {} into tempdir", url));
            progress.step_start("clone catalyst");
            clone_catalyst(&source, &target, sink)?;
            progress.step_done("clone catalyst");
            target
        }
    };

    progress.step_start("rsync vendored paths");
    let mut copied = 0usize;
    for rel in VENDORED_PATHS {
        let src = staging.join(rel);
        if !src.exists() {
            sink.debug(format!("skip {} (not in catalyst)", rel));
            continue;
        }
        let dst = project_root.join(rel);
        wipe_then_copy(&src, &dst)?;
        copied += 1;
        sink.info(format!("synced {}", rel));
    }
    progress.step_done("rsync vendored paths");

    sink.success(format!("blast sync: {} path(s) synced from catalyst", copied));
    Ok(())
}

fn clone_catalyst(source: &Source, target: &Path, sink: &mut dyn Sink) -> BlastResult<()> {
    let url = match source {
        Source::Git { url, .. } => url.clone(),
        Source::LocalCopy { path, .. } => path.to_string_lossy().into_owned(),
    };
    let branch = match source {
        Source::Git { branch, .. } => branch.as_str(),
        Source::LocalCopy { branch, .. } => branch.as_str(),
    };

    sink.debug(format!("git clone --branch {} {} {}", branch, url, target.display()));

    let status = Command::new("git")
        .args(["clone", "--branch", branch, "--no-hardlinks", "--single-branch", "--depth", "1"])
        .arg(&url)
        .arg(target)
        .status()
        .map_err(|e| BlastError::Project(format!("failed to spawn `git clone`: {}", e)))?;

    match status.success() {
        true => Ok(()),
        false => Err(BlastError::Project(format!(
            "git clone failed (exit {}): branch {} from {}",
            status.code().unwrap_or(-1), // allow: signal-killed exit has no code; -1 sentinel
            branch,
            url
        ))),
    }
}

/// Bulk-overwrite `dst` with `src`. Wipes `dst` first if it exists so removed
/// upstream files don't linger locally. Single file → file copy; directory →
/// recursive copy.
fn wipe_then_copy(src: &Path, dst: &Path) -> BlastResult<()> {
    if src.is_file() {
        match dst.parent() {
            Some(parent) => std::fs::create_dir_all(parent)?,
            None => {} // allow: dst is filesystem root, no parent to create
        }
        if dst.exists() {
            std::fs::remove_file(dst)?;
        }
        std::fs::copy(src, dst)?;
        return Ok(());
    }
    if src.is_dir() {
        if dst.exists() {
            std::fs::remove_dir_all(dst)?;
        }
        std::fs::create_dir_all(dst)?;
        copy_dir_contents(src, dst)?;
        return Ok(());
    }
    Err(BlastError::Project(format!("source path is neither file nor dir: {}", src.display())))
}

fn copy_dir_contents(src: &Path, dst: &Path) -> BlastResult<()> {
    for entry_res in std::fs::read_dir(src)? {
        let entry = entry_res?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let kind = entry.file_type()?;
        match kind.is_dir() {
            true => {
                std::fs::create_dir_all(&to)?;
                copy_dir_contents(&from, &to)?;
            }
            false => {
                std::fs::copy(&from, &to)?;
            }
        }
    }
    Ok(())
}
