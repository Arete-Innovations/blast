use crate::error::BlastResult;
use crate::governor::config::GovernorConfig;
use crate::governor::report::format_report;
use crate::governor::scanner::scan_project;
use std::path::Path;

pub struct RunOutcome {
    pub violation_count: usize,
    pub output: String,
}

pub fn run_check(project_root: &Path, verbose: bool) -> BlastResult<RunOutcome> {
    let blueprint_ir = project_root.join("target/blueprint/fe_lint.json");
    let config = GovernorConfig::load_or_default(&blueprint_ir)?;
    let report = scan_project(project_root, &config)?;
    let output = format_report(&report.violations, report.files_scanned, verbose);
    Ok(RunOutcome {
        violation_count: report.violations.len(),
        output,
    })
}
