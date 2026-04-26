use crate::state::FeLintState;
use crate::governor::rules::helpers::{is_comment_line, line_has_allow, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref ANY_RE: Regex = match Regex::new(r":\s*any\b") {
        Ok(r) => r,
        Err(_re_err) => panic!("TypeAny regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct TypeAny;

impl TypeAny {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for TypeAny {
    fn name(&self) -> &'static str {
        "TypeAny"
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
        if line_has_allow(line, "@allow-any") {
            return None;
        }
        if !ANY_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "TypeAny",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "give a real type or annotate `// @allow-any` if truly unavoidable",
        ))
    }
}
