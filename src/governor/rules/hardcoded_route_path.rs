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
    static ref ROUTER_PUSH_RE: Regex = match Regex::new(
        r#"router\s*\.\s*(push|replace)\s*\(\s*['"`](/[^'"`]*)['"`]"#
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("HardcodedRoutePath router regex failed to compile"), // allow: const pattern, infallible
    };
    static ref ROUTER_LINK_TO_RE: Regex = match Regex::new(
        r#"<router-link[^>]*\sto\s*=\s*['"`](/[^'"`]*)['"`]"#
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("HardcodedRoutePath router-link regex failed to compile"), // allow: const pattern, infallible
    };
    static ref ANCHOR_HREF_RE: Regex = match Regex::new(
        r#"<a[^>]*\shref\s*=\s*['"`](/[^'"`]*)['"`]"#
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("HardcodedRoutePath anchor regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct HardcodedRoutePath;

impl HardcodedRoutePath {
    pub fn new() -> Self {
        Self
    }
}

fn line_violates(line: &str) -> bool {
    if ROUTER_PUSH_RE.is_match(line) {
        return true;
    }
    if ROUTER_LINK_TO_RE.is_match(line) {
        return true;
    }
    if ANCHOR_HREF_RE.is_match(line) {
        return true;
    }
    false
}

impl Rule for HardcodedRoutePath {
    fn name(&self) -> &'static str {
        "HardcodedRoutePath"
    }

    fn check(&self, file: &Path, line: &str, line_no: usize, _config: &FeLintState) -> Option<Violation> {
        if is_comment_line(line) {
            return None;
        }
        if !line_violates(line) {
            return None;
        }
        Some(Violation::new(
            "HardcodedRoutePath",
            file.to_path_buf(),
            line_no,
            snippet_of(line),
            "use a named route from generated/router/route-names.ts (e.g. { name: 'users.list' })",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn run(line: &str) -> Option<Violation> {
        let rule = HardcodedRoutePath::new();
        let cfg = FeLintState::default();
        rule.check(&PathBuf::from("frontend/src/pages/X.vue"), line, 1, &cfg)
    }

    #[test]
    fn flags_router_push_string() {
        let v = run(r#"  router.push('/users/42')"#);
        assert!(v.is_some(), "expected violation, got none");
    }

    #[test]
    fn flags_router_link_to_string() {
        let v = run(r#"<router-link to="/orders" />"#);
        assert!(v.is_some(), "expected violation, got none");
    }

    #[test]
    fn flags_anchor_with_internal_path() {
        let v = run(r#"<a href="/dashboard">go</a>"#);
        assert!(v.is_some(), "expected violation, got none");
    }

    #[test]
    fn allows_named_route_object() {
        let v = run(r#"router.push({ name: 'users.detail', params: { id: 42 } })"#);
        assert!(v.is_none(), "expected clean, got {:?}", v);
    }

    #[test]
    fn allows_router_link_named_object() {
        let v = run(r#"<router-link :to="{ name: 'orders.list' }" />"#);
        assert!(v.is_none(), "expected clean, got {:?}", v);
    }

    #[test]
    fn ignores_external_anchor() {
        let v = run(r#"<a href="https://example.com">x</a>"#);
        assert!(v.is_none(), "external https url should not be flagged");
    }
}
