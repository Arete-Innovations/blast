use crate::governor::rules::helpers::{
    extension_is, extract_template_block, path_contains, snippet_of,
};
use crate::governor::rules::traits::FileRule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use std::path::Path;

pub struct PageShellRequired;

impl PageShellRequired {
    pub fn new() -> Self {
        Self
    }
}

fn is_page_file(file: &Path) -> bool {
    if !extension_is(file, "vue") {
        return false;
    }
    path_contains(file, "/pages/")
}

/// Strip HTML/Vue comments `<!-- ... -->` from a template body.
fn strip_comments(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 4 <= bytes.len() && &bytes[i..i + 4] == b"<!--" {
            // Skip through `-->` if found, else through end.
            let rest = &body[i + 4..];
            match rest.find("-->") {
                Some(end_rel) => {
                    i = i + 4 + end_rel + 3;
                }
                None => {
                    i = bytes.len();
                }
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Returns the first non-whitespace tag-name in the template body, or None
/// when the body has no tags or is whitespace-only.
fn first_tag_name(body: &str) -> Option<String> {
    let cleaned = strip_comments(body);
    let trimmed = cleaned.trim_start();
    if !trimmed.starts_with('<') {
        return None;
    }
    let after_lt = &trimmed[1..];
    let mut name = String::new();
    for ch in after_lt.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            name.push(ch);
        } else {
            break;
        }
    }
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

impl FileRule for PageShellRequired {
    fn name(&self) -> &'static str {
        "PageShellRequired"
    }

    fn check_file(
        &self,
        file: &Path,
        contents: &str,
        _config: &FeLintState,
    ) -> Vec<Violation> {
        if !is_page_file(file) {
            return Vec::new();
        }
        let block = match extract_template_block(contents) {
            Some(b) => b,
            None => {
                return vec![Violation::new(
                    "PageShellRequired",
                    file.to_path_buf(),
                    1,
                    snippet_of("<template> block missing"),
                    "page components must root with <PageShell layout=\"...\">",
                )];
            }
        };
        match first_tag_name(block.inner) {
            None => vec![Violation::new(
                "PageShellRequired",
                file.to_path_buf(),
                block.start_line,
                snippet_of("<template> is empty"),
                "page components must root with <PageShell layout=\"...\">",
            )],
            Some(tag) => {
                if tag == "PageShell" {
                    Vec::new()
                } else {
                    vec![Violation::new(
                        "PageShellRequired",
                        file.to_path_buf(),
                        block.start_line,
                        snippet_of(&format!("template root is <{tag}>")),
                        "page components must root with <PageShell layout=\"...\"> (cards/split/table/bleed/tabbed)",
                    )]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(file: &str, contents: &str) -> Vec<Violation> {
        let rule = PageShellRequired::new();
        let cfg = FeLintState::default();
        rule.check_file(&PathBuf::from(file), contents, &cfg)
    }

    #[test]
    fn flags_page_with_div_root() {
        let src = r#"
<template>
  <div>hello</div>
</template>
"#;
        let v = run("frontend/src/custom/pages/UsersPage.vue", src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn allows_page_with_pageshell_root() {
        let src = r#"
<template>
  <PageShell layout="cards">
    <div>x</div>
  </PageShell>
</template>
"#;
        let v = run("frontend/src/custom/pages/UsersPage.vue", src);
        assert!(v.is_empty(), "got {:?}", v);
    }

    #[test]
    fn ignores_non_page_files() {
        let src = r#"<template><div /></template>"#;
        let v = run("frontend/src/custom/components/UserCard.vue", src);
        assert!(v.is_empty());
    }

    #[test]
    fn flags_empty_template() {
        let src = r#"<template>   </template>"#;
        let v = run("frontend/src/pages/UsersPage.vue", src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn flags_comment_only_template() {
        let src = r#"<template>
  <!-- nothing here yet -->
</template>"#;
        let v = run("frontend/src/pages/UsersPage.vue", src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn allows_pageshell_with_attributes_and_whitespace() {
        let src = r#"<template>

   <PageShell
       layout="split"
       :data-foo="x">
       child
   </PageShell>
</template>"#;
        let v = run("frontend/src/pages/X.vue", src);
        assert!(v.is_empty(), "got {:?}", v);
    }

    #[test]
    fn flags_main_root() {
        let src = r#"<template><main>hi</main></template>"#;
        let v = run("frontend/src/pages/X.vue", src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn handles_missing_template_block() {
        let src = r#"<script setup>const x = 1</script>"#;
        let v = run("frontend/src/pages/X.vue", src);
        assert_eq!(v.len(), 1);
    }
}
