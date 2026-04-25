use crate::state::FeLintState;
use crate::governor::rules::helpers::{is_comment_line, rel_path_str, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref REM_RE: Regex = match Regex::new(r"\b\d+(\.\d+)?rem\b") {
        Ok(r) => r,
        Err(_re_err) => panic!("RawRemOutsideTokens regex failed to compile"),
    };
}

const ALLOW_CONTEXTS: &[&str] = &[
    "minmax(",
    "@media",
    "letter-spacing",
    "filter:",
    "filter ",
    "blur(",
    "backdrop-filter",
    "background-size",
    "box-shadow",
];

pub struct RawRemOutsideTokens;

impl RawRemOutsideTokens {
    pub fn new() -> Self {
        Self
    }
}

fn is_in_tokens_file(file: &Path, tokens_path: &str) -> bool {
    let path = rel_path_str(file);
    let normalized = tokens_path.replace('\\', "/");
    path.ends_with(&normalized)
}

fn line_is_allow_context(line: &str) -> bool {
    ALLOW_CONTEXTS.iter().any(|ctx| line.contains(ctx))
}

impl Rule for RawRemOutsideTokens {
    fn name(&self) -> &'static str {
        "RawRemOutsideTokens"
    }

    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        config: &FeLintState,
    ) -> Option<Violation> {
        if is_in_tokens_file(file, &config.tokens_file) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        if line_is_allow_context(line) {
            return None;
        }
        if !REM_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "RawRemOutsideTokens",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "use var(--app-*) token instead of literal rem",
        ))
    }
}
