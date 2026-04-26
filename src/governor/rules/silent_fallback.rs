use crate::state::FeLintState;
use crate::governor::rules::helpers::{is_comment_line, line_has_allow, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref FALLBACK_OR_RE: Regex = match Regex::new(r#"\|\|\s*(['"][^'"]*['"]|\d+|true|false)"#) {
        Ok(r) => r,
        Err(_re_err) => panic!("SilentFallback || regex failed to compile"), // allow: const pattern, infallible
    };
    static ref FALLBACK_NULLISH_RE: Regex = match Regex::new(r"\?\?\s*(\{\s*\}|\[\s*\])") {
        Ok(r) => r,
        Err(_re_err) => panic!("SilentFallback ?? regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct SilentFallback;

impl SilentFallback {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for SilentFallback {
    fn name(&self) -> &'static str {
        "SilentFallback"
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
        if line_has_allow(line, "@allow-fallback") {
            return None;
        }
        if !FALLBACK_OR_RE.is_match(line) && !FALLBACK_NULLISH_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "SilentFallback",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "handle the missing case explicitly; mark `// @allow-fallback` if intentional",
        ))
    }
}
