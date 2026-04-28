use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    governor::{
        rules::{
            helpers::{extension_is, extract_template_block, path_contains, snippet_of},
            traits::FileRule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

lazy_static! {
    /// Match a `<PageShell ...>` opening tag and capture its attribute span.
    static ref PAGESHELL_RE: Regex = match Regex::new(r"(?s)<PageShell\b([^>]*)>") {
        Ok(r) => r,
        Err(_re_err) => panic!("InlineLayoutProps PageShell regex failed to compile"), // allow: const pattern, infallible
    };
    /// Match banned attribute names (literal or v-bind shorthand `:`).
    static ref BANNED_ATTR_RE: Regex = match Regex::new(
        r#"(?:^|\s)(?::)?(padding|margin|gap|width|height)\s*="#
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("InlineLayoutProps banned-attr regex failed to compile"), // allow: const pattern, infallible
    };
    /// Match top-level inline declarations inside a scoped <style> block of a
    /// page component. We look at lines containing `padding:`, `margin:`,
    /// `gap:`. Heuristic — full CSS parsing is out of scope.
    static ref CSS_BANNED_RE: Regex = match Regex::new(
        r"(?m)^\s*(padding|margin|gap)\s*:"
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("InlineLayoutProps css-banned regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct InlineLayoutProps;

impl InlineLayoutProps {
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

fn line_of_offset(contents: &str, offset: usize) -> usize {
    contents[..offset.min(contents.len())].lines().count().max(1)
}

/// Return the body text and starting line of every `<style ... scoped ...>`
/// block in the SFC.
fn scoped_style_blocks(contents: &str) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let lower = contents.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find("<style") {
        let open = cursor + rel;
        let after = open + "<style".len();
        let gt_rel = match lower[after..].find('>') {
            Some(i) => i,
            None => break,
        };
        let attrs = &lower[after..after + gt_rel];
        let body_start = after + gt_rel + 1;
        let close_rel = match lower[body_start..].find("</style") {
            Some(i) => i,
            None => break,
        };
        let body = contents[body_start..body_start + close_rel].to_string();
        let line_no = line_of_offset(contents, body_start);
        if attrs.contains("scoped") {
            out.push((body, line_no));
        }
        cursor = body_start + close_rel;
    }
    out
}

impl FileRule for InlineLayoutProps {
    fn name(&self) -> &'static str {
        "InlineLayoutProps"
    }

    fn check_file(&self, file: &Path, contents: &str, _config: &FeLintState) -> Vec<Violation> {
        if !is_page_file(file) {
            return Vec::new();
        }
        let mut out: Vec<Violation> = Vec::new();

        // Pass 1: PageShell attribute scan inside <template>.
        let template_block = extract_template_block(contents);
        match template_block {
            Some(block) => {
                for caps in PAGESHELL_RE.captures_iter(block.inner) {
                    let attrs = match caps.get(1) {
                        Some(m) => m.as_str(),
                        None => continue,
                    };
                    let bad_caps = match BANNED_ATTR_RE.captures(attrs) {
                        Some(c) => c,
                        None => continue,
                    };
                    let attr_name = match bad_caps.get(1) {
                        Some(m) => m.as_str().to_string(),
                        None => continue,
                    };
                    let inner_offset = match caps.get(0) {
                        Some(m) => m.start(),
                        None => continue, // captures_iter always yields a match, defensive only
                    };
                    let line_no = block.start_line + block.inner[..inner_offset].matches('\n').count();
                    out.push(Violation::new(
                        "InlineLayoutProps",
                        file.to_path_buf(),
                        line_no,
                        snippet_of(&format!("<PageShell {attr_name}=...>")),
                        "PageShell does not accept padding/margin/gap/width/height — pick a layout instead",
                    ));
                }
            }
            None => {}
        }

        // Pass 2: scoped <style> blocks of page components.
        for (body, body_start_line) in scoped_style_blocks(contents) {
            for cap in CSS_BANNED_RE.captures_iter(&body) {
                let m = match cap.get(0) {
                    Some(m) => m,
                    None => continue,
                };
                let line_in_body = body[..m.start()].matches('\n').count();
                out.push(Violation::new(
                    "InlineLayoutProps",
                    file.to_path_buf(),
                    body_start_line + line_in_body,
                    snippet_of(m.as_str().trim()),
                    "padding/margin/gap belong to the layout, not the page's scoped style",
                ));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn run(file: &str, contents: &str) -> Vec<Violation> {
        let rule = InlineLayoutProps::new();
        let cfg = FeLintState::default();
        rule.check_file(&PathBuf::from(file), contents, &cfg)
    }

    #[test]
    fn flags_padding_prop_on_pageshell() {
        let src = r#"<template>
  <PageShell layout="cards" padding="10">x</PageShell>
</template>"#;
        let v = run("frontend/src/pages/X.vue", src);
        assert!(!v.is_empty());
    }

    #[test]
    fn flags_bound_gap_prop_on_pageshell() {
        let src = r#"<template>
  <PageShell layout="cards" :gap="x">x</PageShell>
</template>"#;
        let v = run("frontend/src/pages/X.vue", src);
        assert!(!v.is_empty());
    }

    #[test]
    fn allows_layout_only() {
        let src = r#"<template>
  <PageShell layout="cards">x</PageShell>
</template>"#;
        let v = run("frontend/src/pages/X.vue", src);
        assert!(v.is_empty(), "got {:?}", v);
    }

    #[test]
    fn flags_top_level_padding_in_scoped_style() {
        let src = r#"<template><PageShell layout="cards"></PageShell></template>
<style scoped>
.foo {
  padding: 1rem;
}
</style>"#;
        let v = run("frontend/src/pages/X.vue", src);
        assert!(!v.is_empty());
    }

    #[test]
    fn ignores_non_page_files() {
        let src = r#"<template><PageShell padding="10"/></template>"#;
        let v = run("frontend/src/components/Foo.vue", src);
        assert!(v.is_empty());
    }
}
