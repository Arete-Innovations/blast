use crate::governor::rules::helpers::{is_comment_line, path_contains, snippet_of};
use crate::governor::rules::traits::Rule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    static ref STORAGE_RE: Regex = match Regex::new(
        r"\blocalStorage\.|\bsessionStorage\.|\bindexedDB\."
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("LocalStorageOutsidePersistence regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct LocalStorageOutsidePersistence;

impl LocalStorageOutsidePersistence {
    pub fn new() -> Self {
        Self
    }
}

fn is_persistence_dir(file: &Path) -> bool {
    path_contains(file, "/persistence/") || path_contains(file, "composables/auth.ts")
}

impl Rule for LocalStorageOutsidePersistence {
    fn name(&self) -> &'static str {
        "LocalStorageOutsidePersistence"
    }

    fn check(
        &self,
        file: &Path,
        line: &str,
        line_no: usize,
        _config: &FeLintState,
    ) -> Option<Violation> {
        if is_persistence_dir(file) {
            return None;
        }
        if is_comment_line(line) {
            return None;
        }
        if !STORAGE_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "LocalStorageOutsidePersistence",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "browser storage belongs in a persistence/ directory",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(file: &str, line: &str) -> Option<Violation> {
        let rule = LocalStorageOutsidePersistence::new();
        let cfg = FeLintState::default();
        rule.check(&PathBuf::from(file), line, 1, &cfg)
    }

    #[test]
    fn flags_localstorage_in_component() {
        let v = run(
            "frontend/src/components/Foo.vue",
            "localStorage.setItem('k','v')",
        );
        assert!(v.is_some());
    }

    #[test]
    fn flags_sessionstorage_in_composable() {
        let v = run(
            "frontend/src/composables/useX.ts",
            "  const x = sessionStorage.getItem('k')",
        );
        assert!(v.is_some());
    }

    #[test]
    fn allows_in_persistence_dir() {
        let v = run(
            "frontend/src/persistence/local.ts",
            "localStorage.setItem('k','v')",
        );
        assert!(v.is_none());
    }

    #[test]
    fn allows_in_generated_persistence_dir() {
        let v = run(
            "frontend/src/generated/persistence/store.ts",
            "indexedDB.open('db')",
        );
        assert!(v.is_none());
    }

    #[test]
    fn allows_in_auth_composable() {
        let v = run(
            "frontend/src/composables/auth.ts",
            "localStorage.setItem('auth_token', token)",
        );
        assert!(v.is_none());
    }
}
