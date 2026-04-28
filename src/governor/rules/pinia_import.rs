use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    governor::{
        rules::{
            helpers::{is_comment_line, snippet_of},
            traits::Rule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

lazy_static! {
    static ref PINIA_RE: Regex = match Regex::new(
        r#"\bimport\b[^;]*\bfrom\s+['"]pinia['"]"#
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("PiniaImport regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct PiniaImport;

impl PiniaImport {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for PiniaImport {
    fn name(&self) -> &'static str {
        "PiniaImport"
    }

    fn check(&self, file: &Path, line: &str, line_no: usize, _config: &FeLintState) -> Option<Violation> {
        if is_comment_line(line) {
            return None;
        }
        if !PINIA_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "PiniaImport",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "Catablast does not use Pinia — codegen'd composables are the store",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn run(line: &str) -> Option<Violation> {
        let rule = PiniaImport::new();
        let cfg = FeLintState::default();
        rule.check(&PathBuf::from("frontend/src/x.ts"), line, 1, &cfg)
    }

    #[test]
    fn flags_default_import() {
        let v = run("import { defineStore } from 'pinia'");
        assert!(v.is_some());
    }

    #[test]
    fn flags_namespace_import() {
        let v = run("import * as pinia from 'pinia'");
        assert!(v.is_some());
    }

    #[test]
    fn allows_unrelated_import() {
        let v = run("import { ref } from 'vue'");
        assert!(v.is_none());
    }

    #[test]
    fn allows_string_mention_in_comment() {
        let v = run("// pinia is forbidden here");
        assert!(v.is_none());
    }
}
