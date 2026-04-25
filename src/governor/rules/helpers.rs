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
