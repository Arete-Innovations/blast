use crate::governor::violation::Violation;
use crate::state::FeLintState;
use std::path::Path;

pub trait Rule: Sync + Send {
    fn name(&self) -> &'static str;
    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        config: &FeLintState,
    ) -> Option<Violation>;
}

pub trait FileRule: Sync + Send {
    fn name(&self) -> &'static str;
    fn check_file(
        &self,
        file: &Path,
        contents: &str,
        config: &FeLintState,
    ) -> Vec<Violation>;
}
