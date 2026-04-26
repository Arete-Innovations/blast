use crate::governor::rules::helpers::{is_comment_line, line_has_allow, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref LOADING_RE: Regex = match Regex::new(r#"v-if\s*=\s*"[^"]*[Ll]oading[^"]*""#) {
        Ok(r) => r,
        Err(_re_err) => panic!("LoadingSpinnerAfterFirstLoad regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct LoadingSpinnerAfterFirstLoad;

impl LoadingSpinnerAfterFirstLoad {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for LoadingSpinnerAfterFirstLoad {
    fn name(&self) -> &'static str {
        "LoadingSpinnerAfterFirstLoad"
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
        if line_has_allow(line, "@allow-spinner") {
            return None;
        }
        if !LOADING_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "LoadingSpinnerAfterFirstLoad",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "no spinner after first load — composables refetch silently; mark `// @allow-spinner` for genuine action overlays",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(line: &str) -> Option<Violation> {
        let rule = LoadingSpinnerAfterFirstLoad::new();
        let cfg = FeLintState::default();
        rule.check(&PathBuf::from("frontend/src/custom/pages/X.vue"), line, 1, &cfg)
    }

    #[test]
    fn flags_v_if_loading() {
        let v = run(r#"<Spinner v-if="isLoading" />"#);
        assert!(v.is_some());
    }

    #[test]
    fn flags_v_if_loading_caps() {
        let v = run(r#"<Spinner v-if="userLoading" />"#);
        assert!(v.is_some());
    }

    #[test]
    fn allows_when_marked_allow_spinner() {
        let v = run(r#"<Spinner v-if="isLoading" /> // @allow-spinner"#);
        assert!(v.is_none(), "got {:?}", v);
    }

    #[test]
    fn allows_unrelated_v_if() {
        let v = run(r#"<div v-if="user">x</div>"#);
        assert!(v.is_none(), "got {:?}", v);
    }
}
