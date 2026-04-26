use crate::state::FeLintState;
use crate::governor::rules::helpers::{is_comment_line, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref INLINE_RE: Regex = match Regex::new(r#"(?:\s|^)(?::style\s*=|style\s*=)\s*["']"#) {
        Ok(r) => r,
        Err(_re_err) => panic!("InlineStyle regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct InlineStyle;

impl InlineStyle {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for InlineStyle {
    fn name(&self) -> &'static str {
        "InlineStyle"
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
        if !INLINE_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "InlineStyle",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "move style into <style scoped> via a class",
        ))
    }
}
