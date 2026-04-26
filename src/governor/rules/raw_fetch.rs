use crate::governor::rules::helpers::{is_comment_line, path_contains, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref FETCH_RE: Regex = match Regex::new(
        r"\bfetch\s*\(|\baxios\.|\bnew\s+XMLHttpRequest\b|\bXMLHttpRequest\s*\("
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("RawFetchOutsideApi regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct RawFetchOutsideApi;

impl RawFetchOutsideApi {
    pub fn new() -> Self {
        Self
    }
}

fn is_in_generated_api(file: &Path) -> bool {
    path_contains(file, "/generated/api/")
}

impl Rule for RawFetchOutsideApi {
    fn name(&self) -> &'static str {
        "RawFetchOutsideApi"
    }

    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        _config: &FeLintState,
    ) -> Option<Violation> {
        if is_in_generated_api(file) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        if !FETCH_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "RawFetchOutsideApi",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "use the codegen'd typed client from frontend/src/generated/api/",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(file: &str, line: &str) -> Option<Violation> {
        let rule = RawFetchOutsideApi::new();
        let cfg = FeLintState::default();
        rule.check(&PathBuf::from(file), line, 1, &cfg)
    }

    #[test]
    fn flags_fetch_in_custom() {
        let v = run(
            "frontend/src/custom/composables/useFoo.ts",
            "  const r = await fetch('/api/x')",
        );
        assert!(v.is_some());
    }

    #[test]
    fn flags_axios_in_custom() {
        let v = run(
            "frontend/src/custom/components/Foo.vue",
            "axios.get('/x')",
        );
        assert!(v.is_some());
    }

    #[test]
    fn allows_fetch_in_generated_api() {
        let v = run(
            "frontend/src/generated/api/users.ts",
            "  const r = await fetch('/api/x')",
        );
        assert!(v.is_none());
    }
}
