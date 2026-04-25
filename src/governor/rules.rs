use crate::governor::config::GovernorConfig;
use crate::governor::violation::Violation;
use std::path::Path;

pub trait Rule: Sync + Send {
    fn name(&self) -> &str;
    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        config: &GovernorConfig,
    ) -> Option<Violation>;
}

pub trait FileRule: Sync + Send {
    fn name(&self) -> &str;
    fn check_file(
        &self,
        file: &Path,
        contents: &str,
        config: &GovernorConfig,
    ) -> Vec<Violation>;
}

pub fn run_all(_file: &Path, _contents: &str, _config: &GovernorConfig) -> Vec<Violation> {
    Vec::new()
}
