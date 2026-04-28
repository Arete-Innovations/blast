use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    governor::{
        rules::{
            helpers::{is_comment_line, path_contains, snippet_of},
            traits::Rule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

lazy_static! {
    /// Matches `const page = ref(...)`, `const sort = ref(...)`,
    /// `const filter = ref(...)`, `const pageSize = ref(...)`.
    static ref LIST_LOCAL_RE: Regex = match Regex::new(
        r"\b(?:const|let|var)\s+(page|page_size|pageSize|sort|filter)\s*=\s*ref\s*\("
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("LocalListState regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct LocalListState;

impl LocalListState {
    pub fn new() -> Self {
        Self
    }
}

fn is_list_view_file(file: &Path) -> bool {
    let path = crate::governor::rules::helpers::rel_path_str(file);
    let name = match path.rsplit('/').next() {
        Some(n) => n.to_ascii_lowercase(),
        None => return false,
    };
    if !path_contains(file, "/pages/") {
        return false;
    }
    name.contains("list")
}

impl Rule for LocalListState {
    fn name(&self) -> &'static str {
        "LocalListState"
    }

    fn check(&self, file: &Path, line: &str, line_no: usize, _config: &FeLintState) -> Option<Violation> {
        if is_comment_line(line) {
            return None;
        }
        if !is_list_view_file(file) {
            return None;
        }
        if !LIST_LOCAL_RE.is_match(line) {
            return None;
        }
        Some(Violation::new(
            "LocalListState",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "page/sort/filter live in URL — use useUrlListState() or the resource's useXList composable",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn run(file: &str, line: &str) -> Option<Violation> {
        let rule = LocalListState::new();
        let cfg = FeLintState::default();
        rule.check(&PathBuf::from(file), line, 1, &cfg)
    }

    #[test]
    fn flags_local_page_ref_in_list_page() {
        let v = run("frontend/src/pages/UsersListPage.vue", "const page = ref(1)");
        assert!(v.is_some(), "expected violation, got none");
    }

    #[test]
    fn flags_local_sort_ref_in_list_page() {
        let v = run("frontend/src/pages/OrdersListPage.vue", "const sort = ref('-created_at')");
        assert!(v.is_some(), "expected violation, got none");
    }

    #[test]
    fn ignores_local_page_outside_list_page() {
        let v = run("frontend/src/components/Pager.vue", "const page = ref(1)");
        assert!(v.is_none(), "non-list page should not trigger, got {:?}", v);
    }

    #[test]
    fn ignores_destructure_from_composable() {
        let v = run("frontend/src/pages/UsersListPage.vue", "const { data, page, sort, filter } = useUsersList()");
        assert!(v.is_none(), "destructure from composable should be clean");
    }
}
