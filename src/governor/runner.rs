use std::path::Path;

use crate::{
    error::BlastResult,
    governor::{config::load_or_default, report::format_report, scanner::scan_project},
};

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
