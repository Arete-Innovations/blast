use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    governor::{
        rules::{
            helpers::{file_in_list, is_comment_line, snippet_of},
            traits::Rule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

lazy_static! {
    static ref PX_RE: Regex = match Regex::new(r"\b\d+(\.\d+)?px\b") {
        Ok(r) => r,
        Err(_re_err) => panic!("HardcodedPx regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct HardcodedPx;

impl HardcodedPx {
    pub fn new() -> Self {
        Self
    }
}

fn line_is_exempt(line: &str, hairline_token: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.contains("@media") {
        return true;
    }
    if trimmed.contains("rootMargin") {
        return true;
    }
    if trimmed.contains(hairline_token) {
        return true;
    }
    false
}

impl Rule for HardcodedPx {
    fn name(&self) -> &'static str {
        "HardcodedPx"
    }

    fn check(&self, file: &Path, line: &str, line_no: usize, config: &FeLintState) -> Option<Violation> {
        if file_in_list(file, &config.exempt_px_files) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        if line_is_exempt(line, &config.hairline_border_rem) {
            return None;
        }
        if !PX_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "HardcodedPx",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "use rem-based token (var(--app-space-*) / hairline 0.0625rem)",
        ))
    }
}
