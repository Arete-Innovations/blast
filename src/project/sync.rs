use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{BlastError, BlastResult};
use crate::io::traits::{Progress, ProgressExt, Sink, SinkExt};
use crate::project::scaffold::Source;

const VENDORED_PATHS: &[&str] = &[
    "src/services/vendored",
    "src/structs/vendored",
    "src/views/vendored",
    "build.rs",
    "catalyst-derive",
];

pub fn run_sync(dev: bool, dry_run: bool, sink: &mut dyn Sink, progress: &mut dyn Progress) -> BlastResult<()> {
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
    let staging: PathBuf = match &source {
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

    let step_label = match dry_run {
        true => "dry-run diff vendored paths",
        false => "rsync vendored paths",
    };
    progress.step_start(step_label);
    let mut touched = 0usize;
    for rel in VENDORED_PATHS {
        let src = staging.join(rel);
        if !src.exists() {
            sink.debug(format!("skip {} (not in catalyst)", rel));
            continue;
        }
        let dst = project_root.join(rel);
        match dry_run {
            true => {
                touched += dry_run_path(&src, &dst, rel, sink)?;
            }
            false => {
                wipe_then_copy(&src, &dst)?;
                touched += 1;
                sink.info(format!("synced {}", rel));
            }
        }
    }
    progress.step_done(step_label);

    match dry_run {
        true => sink.success(format!("blast sync --dry-run: {} file(s) would change (nothing was written)", touched)),
        false => sink.success(format!("blast sync: {} path(s) synced from catalyst", touched)),
    }
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

fn dry_run_path(src: &Path, dst: &Path, rel: &str, sink: &mut dyn Sink) -> BlastResult<usize> {
    if src.is_file() {
        return dry_run_file(src, dst, rel, sink);
    }
    if !src.is_dir() {
        return Err(BlastError::Project(format!("source path is neither file nor dir: {}", src.display())));
    }
    let src_paths = collect_relative_paths(src)?;
    let dst_paths = match dst.exists() {
        true => collect_relative_paths(dst)?,
        false => BTreeSet::new(),
    };
    let mut changed = 0usize;
    for f in &src_paths {
        let src_file = src.join(f);
        let dst_file = dst.join(f);
        let rel_label = format!("{}/{}", rel, f.display());
        match dst_file.exists() {
            true => match files_equal(&src_file, &dst_file)? {
                true => sink.debug(format!("  [same]   {}", rel_label)),
                false => {
                    sink.info(format!("  [WRITE]  {}", rel_label));
                    changed += 1;
                }
            },
            false => {
                sink.info(format!("  [CREATE] {}", rel_label));
                changed += 1;
            }
        }
    }
    for f in dst_paths.difference(&src_paths) {
        sink.info(format!("  [DELETE] {}/{}", rel, f.display()));
        changed += 1;
    }
    Ok(changed)
}

fn dry_run_file(src: &Path, dst: &Path, rel: &str, sink: &mut dyn Sink) -> BlastResult<usize> {
    match dst.exists() {
        true => match files_equal(src, dst)? {
            true => {
                sink.debug(format!("  [same]   {}", rel));
                Ok(0)
            }
            false => {
                sink.info(format!("  [WRITE]  {}", rel));
                Ok(1)
            }
        },
        false => {
            sink.info(format!("  [CREATE] {}", rel));
            Ok(1)
        }
    }
}

fn collect_relative_paths(root: &Path) -> BlastResult<BTreeSet<PathBuf>> {
    let mut out = BTreeSet::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, current: &Path, out: &mut BTreeSet<PathBuf>) -> BlastResult<()> {
    for entry_res in std::fs::read_dir(current)? {
        let entry = entry_res?;
        let path = entry.path();
        let kind = entry.file_type()?;
        match kind.is_dir() {
            true => walk(root, &path, out)?,
            false => {
                let rel = path.strip_prefix(root).map_err(|e| {
                    BlastError::Project(format!("strip_prefix({}, {}): {}", path.display(), root.display(), e))
                })?;
                out.insert(rel.to_path_buf());
            }
        }
    }
    Ok(())
}

fn files_equal(a: &Path, b: &Path) -> BlastResult<bool> {
    let am = std::fs::metadata(a)?;
    let bm = std::fs::metadata(b)?;
    match am.len() == bm.len() {
        false => Ok(false),
        true => {
            let av = std::fs::read(a)?;
            let bv = std::fs::read(b)?;
            Ok(av == bv)
        }
    }
}
