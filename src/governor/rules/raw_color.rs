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
    static ref RAW_COLOR_RE: Regex = match Regex::new(
        r"(#[0-9a-fA-F]{3,8}\b)|(\brgb\s*\()|(\brgba\s*\()|(\bhsl\s*\()|(\bhsla\s*\()"
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("RawColorOutsidePreset regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct RawColorOutsidePreset;

impl RawColorOutsidePreset {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for RawColorOutsidePreset {
    fn name(&self) -> &'static str {
        "RawColorOutsidePreset"
    }

    fn check(&self, file: &Path, line: &str, line_no: usize, config: &FeLintState) -> Option<Violation> {
        if file_in_list(file, &config.exempt_color_files) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        if !RAW_COLOR_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "RawColorOutsidePreset",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "use a token (var(--app-*)) or move color into PrimeVue preset",
        ))
    }
}
