use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    error::BlastResult,
    governor::{rules, violation::Violation, whitelist::Whitelist},
    state::FeLintState,
};

pub struct ScanReport {
    pub violations: Vec<Violation>,
    pub files_scanned: usize,
}

pub fn scan_project(root: &Path, config: &FeLintState) -> BlastResult<ScanReport> {
    let frontend_root = root.join("frontend");
    let whitelist = Whitelist::load(&root.join(".rule_violations_whitelist"))?;

    let targets = collect_targets(&frontend_root)?;
    let files_scanned = targets.len();

    let root_buf = root.to_path_buf();
    let violations: Vec<Violation> = targets.par_iter().flat_map(|path| scan_one(path, &root_buf, config, &whitelist)).collect();

    Ok(ScanReport { violations, files_scanned })
}

fn scan_one(path: &Path, root: &Path, config: &FeLintState, whitelist: &Whitelist) -> Vec<Violation> {
    let raw = match std::fs::read_to_string(path) {
        Ok(v) => v,
        Err(_read_failed) => return Vec::new(),
    };
    let rel: &Path = match path.strip_prefix(root) {
        Ok(stripped) => stripped,
        Err(_no_prefix) => path,
    };
    let mut file_violations = rules::run_all(rel, &raw, config);
    file_violations.retain(|v| !whitelist.suppresses(rel, &v.snippet));
    file_violations.retain(|v| !is_globally_whitelisted(&v.snippet, config));
    file_violations
}

fn collect_targets(root: &Path) -> BlastResult<Vec<PathBuf>> {
    let mut targets: Vec<PathBuf> = Vec::new();
    if !root.is_dir() {
        return Ok(targets);
    }
    for entry in WalkDir::new(root).into_iter() {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if is_target_file(path) {
            targets.push(path.to_path_buf());
        }
    }
    Ok(targets)
}

fn is_target_file(path: &Path) -> bool {
    let ext = match path.extension() {
        Some(e) => e.to_string_lossy().to_string(),
        None => return false,
    };
    matches!(ext.as_str(), "ts" | "vue" | "css")
}

fn is_globally_whitelisted(snippet: &str, config: &FeLintState) -> bool {
    config.whitelist_snippets.iter().any(|w| snippet.contains(w))
}
