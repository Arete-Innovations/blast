use crate::governor::rules::helpers::{is_comment_line, rel_path_str, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;

pub struct IconClassOutsideIconsFile {
    compiled: Mutex<Option<Vec<Regex>>>,
}

impl IconClassOutsideIconsFile {
    pub fn new() -> Self {
        Self {
            compiled: Mutex::new(None),
        }
    }

    fn ensure_compiled(&self, patterns: &BTreeSet<String>) -> Vec<Regex> {
        let mut guard = match self.compiled.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        match guard.clone() {
            Some(existing) => existing,
            None => {
                let mut compiled: Vec<Regex> = Vec::with_capacity(patterns.len());
                for p in patterns {
                    match Regex::new(p) {
                        Ok(r) => compiled.push(r),
                        Err(_compile_err) => continue,
                    }
                }
                *guard = Some(compiled.clone());
                compiled
            }
        }
    }
}

fn is_icons_file(file: &Path, icons_path: &str) -> bool {
    let path = rel_path_str(file);
    let normalized = icons_path.replace('\\', "/");
    path.ends_with(&normalized)
}

impl Rule for IconClassOutsideIconsFile {
    fn name(&self) -> &'static str {
        "IconClassOutsideIconsFile"
    }

    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        config: &FeLintState,
    ) -> Option<Violation> {
        if is_icons_file(file, &config.icons_file) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        let patterns = self.ensure_compiled(&config.icon_class_patterns);
        let matched = patterns.iter().any(|r| r.is_match(line));
        if !matched {
            return None;
        }
        Some(Violation::new(
            "IconClassOutsideIconsFile",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "import the icon name from src/icons.ts instead of inlining classes",
        ))
    }
}
