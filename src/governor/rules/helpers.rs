use std::collections::BTreeSet;
use std::path::Path;

pub fn rel_path_str(file: &Path) -> String {
    file.to_string_lossy().replace('\\', "/")
}

pub fn file_in_list(file: &Path, list: &BTreeSet<String>) -> bool {
    let path = rel_path_str(file);
    list.iter().any(|allowed| {
        let allowed_norm = allowed.replace('\\', "/");
        path.ends_with(&allowed_norm) || path == allowed_norm
    })
}

pub fn snippet_of(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() > 200 {
        format!("{}…", &trimmed[..200])
    } else {
        trimmed.to_string()
    }
}

pub fn line_has_allow(line: &str, marker: &str) -> bool {
    line.contains(marker)
}

pub fn is_comment_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with("*")
}

pub fn extension_is(file: &Path, ext: &str) -> bool {
    match file.extension() {
        Some(e) => e.to_string_lossy() == ext,
        None => false,
    }
}

/// Returns true when the file's normalized path contains the given
/// directory segment (e.g. `frontend/src/pages/`). The needle
/// should NOT start with `/` and SHOULD end with `/` to avoid prefix
/// collisions like `pages` matching `pagesrc`.
pub fn path_contains(file: &Path, needle: &str) -> bool {
    rel_path_str(file).contains(needle)
}

/// Indexes (byte offsets) of the first `<template>` opening tag and the
/// matching `</template>` closing tag inside an SFC. Naive top-level
/// only — does not support nested `<template>` elements outside the
/// outer block correctly, but Vue SFCs cannot have those at the root.
pub struct TemplateBlock<'a> {
    pub inner: &'a str,
    /// 1-based line where the inner content begins.
    pub start_line: usize,
}

pub fn extract_template_block(contents: &str) -> Option<TemplateBlock<'_>> {
    let lower = contents.to_ascii_lowercase();
    let open_idx = lower.find("<template")?;
    // Find end of the opening tag's `>`.
    let after_open_lt = open_idx + "<template".len();
    let rel_gt = lower[after_open_lt..].find('>')?;
    let inner_start = after_open_lt + rel_gt + 1;
    let close_rel = lower[inner_start..].find("</template")?;
    let inner_end = inner_start + close_rel;
    let inner = &contents[inner_start..inner_end];
    let start_line = contents[..inner_start].lines().count();
    Some(TemplateBlock { inner, start_line })
}
