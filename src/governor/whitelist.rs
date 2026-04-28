use std::path::Path;

use crate::error::BlastResult;

pub struct WhitelistEntry {
    pub file_glob: String,
    pub snippet: Option<String>,
}

pub struct Whitelist {
    pub entries: Vec<WhitelistEntry>,
}

impl Whitelist {
    pub fn load(path: &Path) -> BlastResult<Self> {
        if !path.exists() {
            return Ok(Self { entries: Vec::new() });
        }
        let raw = std::fs::read_to_string(path)?;
        let mut entries = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
            let file_glob = parts[0].trim().to_string();
            let snippet = parts.get(1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            entries.push(WhitelistEntry { file_glob, snippet });
        }
        Ok(Self { entries })
    }

    pub fn suppresses(&self, file: &Path, snippet: &str) -> bool {
        let file_str = file.to_string_lossy();
        for entry in &self.entries {
            if !glob_match(&entry.file_glob, &file_str) {
                continue;
            }
            match &entry.snippet {
                Some(needle) => {
                    if snippet.contains(needle) {
                        return true;
                    }
                }
                None => return true,
            }
        }
        false
    }
}

fn glob_match(pattern: &str, target: &str) -> bool {
    let normalized = pattern.replace('\\', "/");
    let target_norm = target.replace('\\', "/");

    let mut regex = String::with_capacity(normalized.len() * 2);
    regex.push('^');
    let bytes = normalized.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let c = bytes[idx] as char;
        if c == '*' {
            let next_is_star = bytes.get(idx + 1).map(|b| *b as char) == Some('*');
            if next_is_star {
                regex.push_str(".*");
                idx += 2;
                if bytes.get(idx).map(|b| *b as char) == Some('/') {
                    idx += 1;
                }
                continue;
            }
            regex.push_str("[^/]*");
            idx += 1;
            continue;
        }
        if matches!(c, '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '?' | '\\') {
            regex.push('\\');
        }
        regex.push(c);
        idx += 1;
    }
    regex.push('$');

    let compiled = match regex::Regex::new(&regex) {
        Ok(r) => r,
        Err(_compile_err) => return false,
    };
    compiled.is_match(&target_norm)
}
