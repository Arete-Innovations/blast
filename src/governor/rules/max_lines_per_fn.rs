use crate::state::FeLintState;
use crate::governor::rules::helpers::{extension_is, snippet_of};
use crate::governor::rules::traits::FileRule;
use crate::governor::violation::Violation;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref FN_START_RE: Regex = match Regex::new(
        r"^\s*(?:export\s+)?(?:async\s+)?function\s+[A-Za-z_$][A-Za-z0-9_$]*\s*\("
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("MaxLinesPerFn fn-start regex failed to compile"), // allow: const pattern, infallible
    };
    static ref ARROW_FN_RE: Regex = match Regex::new(
        r"^\s*(?:export\s+)?(?:const|let|var)\s+[A-Za-z_$][A-Za-z0-9_$]*\s*=\s*(?:async\s*)?\(?[^=]*=>\s*\{"
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("MaxLinesPerFn arrow regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct MaxLinesPerFn;

impl MaxLinesPerFn {
    pub fn new() -> Self {
        Self
    }
}

fn is_function_start(line: &str) -> bool {
    FN_START_RE.is_match(line) || ARROW_FN_RE.is_match(line)
}

fn count_braces(line: &str, depth: &mut i32) {
    for ch in line.chars() {
        if ch == '{' {
            *depth += 1;
        } else if ch == '}' {
            *depth -= 1;
        }
    }
}

fn function_extent(lines: &[&str], start: usize) -> usize {
    let mut depth: i32 = 0;
    let mut found_open = false;
    let mut idx = start;
    while idx < lines.len() {
        let before = depth;
        count_braces(lines[idx], &mut depth);
        if !found_open && depth > before {
            found_open = true;
        }
        if found_open && depth <= 0 {
            return idx - start + 1;
        }
        idx += 1;
    }
    idx - start
}

impl FileRule for MaxLinesPerFn {
    fn name(&self) -> &'static str {
        "MaxLinesPerFn"
    }

    fn check_file(
        &self,
        file: &Path,
        contents: &str,
        config: &FeLintState,
    ) -> Vec<Violation> {
        let is_ts_or_vue = extension_is(file, "ts") || extension_is(file, "vue");
        if !is_ts_or_vue {
            return Vec::new();
        }
        let lines: Vec<&str> = contents.lines().collect();
        let mut out: Vec<Violation> = Vec::new();
        let mut idx = 0usize;
        while idx < lines.len() {
            if !is_function_start(lines[idx]) {
                idx += 1;
                continue;
            }
            let extent = function_extent(&lines, idx);
            if extent > config.max_lines_per_fn {
                let snippet = format!("function spans {} lines", extent);
                let suggestion = format!(
                    "decompose this function; max is {} lines",
                    config.max_lines_per_fn
                );
                out.push(Violation::new(
                    "MaxLinesPerFn",
                    file.to_path_buf(),
                    idx + 1,
                    snippet_of(&snippet),
                    suggestion,
                ));
            }
            idx += extent.max(1);
        }
        out
    }
}
