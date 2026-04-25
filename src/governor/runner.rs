use crate::error::BlastResult;
use crate::governor::config::load_or_default;
use crate::governor::report::format_report;
use crate::governor::scanner::scan_project;
use std::path::Path;

pub struct RunOutcome {
    pub violation_count: usize,
    pub output: String,
}

pub fn run_check(project_root: &Path, verbose: bool) -> BlastResult<RunOutcome> {
    let config = load_or_default(project_root)?;
    let report = scan_project(project_root, &config)?;
    let output = format_report(&report.violations, report.files_scanned, verbose);
    Ok(RunOutcome {
        violation_count: report.violations.len(),
        output,
    })
}
