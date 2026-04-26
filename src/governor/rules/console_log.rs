use crate::state::FeLintState;
use crate::governor::rules::helpers::{is_comment_line, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref CONSOLE_RE: Regex = match Regex::new(r"\bconsole\.(log|warn|error)\s*\(") {
        Ok(r) => r,
        Err(_re_err) => panic!("ConsoleLog regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct ConsoleLog;

impl ConsoleLog {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for ConsoleLog {
    fn name(&self) -> &'static str {
        "ConsoleLog"
    }

    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        _config: &FeLintState,
    ) -> Option<Violation> {
        if is_comment_line(line) {
            return None;
        }
        if !CONSOLE_RE.is_match(line) {
            return None;
        }
        if line.contains("import.meta.env.DEV") {
            return None;
        }
        Some(Violation::new(
            "ConsoleLog",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "wrap with `import.meta.env.DEV && ...` or remove",
        ))
    }
}
