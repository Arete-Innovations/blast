use crate::governor::rules::helpers::{extension_is, path_contains, snippet_of};
use crate::governor::rules::traits::FileRule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    /// Match an interface or type-alias declaration opening brace.
    static ref DECL_OPEN_RE: Regex = match Regex::new(
        r"^\s*(?:export\s+)?(?:interface\s+[A-Za-z_$][A-Za-z0-9_$]*\s*\{|type\s+[A-Za-z_$][A-Za-z0-9_$]*\s*=\s*\{)"
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("SnakeCaseInterfaceFields decl regex failed to compile"), // allow: const pattern, infallible
    };
    /// Match a property line: optional `readonly`, name, optional `?`, then `:`.
    static ref FIELD_RE: Regex = match Regex::new(
        r"^\s*(?:readonly\s+)?([A-Za-z_$][A-Za-z0-9_$]*)\s*\??\s*:"
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("SnakeCaseInterfaceFields field regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct SnakeCaseInterfaceFields;

impl SnakeCaseInterfaceFields {
    pub fn new() -> Self {
        Self
    }
}

fn is_types_dir(file: &Path) -> bool {
    path_contains(file, "/types/")
}

fn is_snake_case(ident: &str) -> bool {
    !ident.chars().any(|c| c.is_ascii_uppercase())
}

impl FileRule for SnakeCaseInterfaceFields {
    fn name(&self) -> &'static str {
        "SnakeCaseInterfaceFields"
    }

    fn check_file(
        &self,
        file: &Path,
        contents: &str,
        _config: &FeLintState,
    ) -> Vec<Violation> {
        if !extension_is(file, "ts") {
            return Vec::new();
        }
        if !is_types_dir(file) {
            return Vec::new();
        }
        let mut out: Vec<Violation> = Vec::new();
        let mut depth: i32 = 0;
        let mut in_decl = false;
        for (idx, line) in contents.lines().enumerate() {
            let line_no = idx + 1;
            let trimmed = line.trim_start();
            // Track if a new interface/type-literal block opens this line.
            if !in_decl && DECL_OPEN_RE.is_match(line) {
                in_decl = true;
                depth = 0;
            }
            // Update brace depth.
            for ch in line.chars() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                }
            }
            if in_decl && depth <= 0 {
                in_decl = false;
            }
            if !in_decl {
                continue;
            }
            // Skip comment lines.
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }
            let caps = match FIELD_RE.captures(line) {
                Some(c) => c,
                None => continue,
            };
            let ident_m = match caps.get(1) {
                Some(m) => m,
                None => continue,
            };
            let ident = ident_m.as_str();
            if ident == "extends" || ident == "implements" {
                continue;
            }
            if !is_snake_case(ident) {
                out.push(Violation::new(
                    "SnakeCaseInterfaceFields",
                    file.to_path_buf(),
                    line_no,
                    snippet_of(line),
                    "interface/type fields must be snake_case (matches Rust serde)",
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(file: &str, contents: &str) -> Vec<Violation> {
        let rule = SnakeCaseInterfaceFields::new();
        let cfg = FeLintState::default();
        rule.check_file(&PathBuf::from(file), contents, &cfg)
    }

    #[test]
    fn flags_camel_case_field() {
        let src = r#"
export interface User {
    id: number;
    firstName: string;
}
"#;
        let v = run("frontend/src/types/user.ts", src);
        assert_eq!(v.len(), 1, "got {:?}", v);
    }

    #[test]
    fn allows_snake_case_fields() {
        let src = r#"
export interface User {
    id: number;
    first_name: string;
    created_at: string;
}
"#;
        let v = run("frontend/src/types/user.ts", src);
        assert!(v.is_empty(), "got {:?}", v);
    }

    #[test]
    fn flags_camel_case_in_type_alias() {
        let src = r#"
export type Order = {
    orderId: string;
    total_cents: number;
};
"#;
        let v = run("frontend/src/generated/types/order.ts", src);
        assert_eq!(v.len(), 1, "got {:?}", v);
    }

    #[test]
    fn ignores_files_outside_types_dirs() {
        let src = r#"
export interface User {
    firstName: string;
}
"#;
        let v = run("frontend/src/composables/useUser.ts", src);
        assert!(v.is_empty(), "got {:?}", v);
    }
}
