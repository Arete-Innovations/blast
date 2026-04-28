//! SQL-first detection of Postgres `CREATE TYPE ... AS ENUM (...)` statements.
//!
//! Walks every `src/database/migrations/*/up.sql` under a project root,
//! extracts each `CREATE TYPE <name> AS ENUM ('v1', 'v2', ...);` block,
//! and returns the parsed result as IR. Multi-line statements, leading
//! whitespace, mixed casing and inline `--` comments are all tolerated.
//!
//! Output IR (`ParsedEnum`) feeds two downstream consumers:
//!   - the per-enum Rust struct + Diesel `FromSql`/`ToSql` codegen
//!   - the (future) FE codegen that emits matching TS enum literal unions

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use regex::Regex;

use crate::error::{BlastError, BlastResult};

/// One parsed Postgres `CREATE TYPE ... AS ENUM (...)` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEnum {
    /// Original snake_case enum type name as written in SQL (`user_role`).
    pub name: String,
    /// Variant string literals in declaration order (`["admin", "member"]`).
    pub variants: Vec<String>,
    /// Absolute path of the migration `up.sql` that declared the enum.
    pub source_file: PathBuf,
}

/// PascalCase a snake/kebab/space-separated identifier without singularizing.
/// `user_role` -> `UserRole`, `status` -> `Status`, `in_progress` -> `InProgress`.
pub fn pascalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut capitalize_next = true;
    for ch in input.chars() {
        if ch == '_' || ch == '-' || ch == ' ' {
            capitalize_next = true;
            continue;
        }
        if !ch.is_ascii_alphanumeric() {
            continue;
        }
        if capitalize_next {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
            capitalize_next = false;
        } else {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        }
    }
    out
}

/// Walk `<project_root>/src/database/migrations/*/up.sql` and return every
/// ENUM type declared across them.
///
/// Duplicate names across migrations are tolerated: the **first**
/// occurrence wins (deterministic by sorted migration directory name).
/// Diesel's own first-migration `_diesel_initial_setup` cannot legally
/// declare custom types, so collisions in practice mean the user copied
/// the same statement into two migrations — we surface a soft warning
/// via the returned `dups` list rather than erroring.
pub fn scan_project_enums(project_root: &Path) -> BlastResult<ScanReport> {
    let migrations_dir = project_root.join("src").join("database").join("migrations");
    if !migrations_dir.is_dir() {
        return Ok(ScanReport::default());
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&migrations_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();

    let mut by_name: BTreeMap<String, ParsedEnum> = BTreeMap::new();
    let mut dups: Vec<String> = Vec::new();

    for dir in &dirs {
        let up_path = dir.join("up.sql");
        if !up_path.is_file() {
            continue;
        }
        let body = fs::read_to_string(&up_path)?;
        for parsed in parse_enums_in_sql(&body, &up_path)? {
            if by_name.contains_key(&parsed.name) {
                dups.push(parsed.name.clone());
                continue;
            }
            by_name.insert(parsed.name.clone(), parsed);
        }
    }

    let mut enums: Vec<ParsedEnum> = by_name.into_values().collect();
    enums.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(ScanReport { enums, duplicates: dups })
}

/// Result of [`scan_project_enums`].
#[derive(Debug, Default, Clone)]
pub struct ScanReport {
    pub enums: Vec<ParsedEnum>,
    /// Names that appeared in more than one migration. The first-seen
    /// declaration wins; this list is informational so callers can warn
    /// the user.
    pub duplicates: Vec<String>,
}

/// Parse all `CREATE TYPE <name> AS ENUM (...)` statements out of a
/// single SQL body string. The `source` path is recorded on each match
/// so callers can point at the offending file in error messages.
pub fn parse_enums_in_sql(body: &str, source: &Path) -> BlastResult<Vec<ParsedEnum>> {
    let cleaned = strip_line_comments(body);
    let re = enum_regex()?;
    let mut out: Vec<ParsedEnum> = Vec::new();
    for caps in re.captures_iter(&cleaned) {
        let name = match caps.get(1) {
            Some(m) => m.as_str().trim().to_string(),
            None => {
                return Err(BlastError::Invalid(format!("CREATE TYPE in {} is missing a type name (regex group 1 absent)", source.display())));
            }
        };
        let body_str = match caps.get(2) {
            Some(m) => m.as_str().to_string(),
            None => {
                return Err(BlastError::Invalid(format!("CREATE TYPE in {} is missing a variant list (regex group 2 absent)", source.display())));
            }
        };
        if name.is_empty() {
            return Err(BlastError::Invalid(format!("CREATE TYPE in {} is missing a type name", source.display())));
        }
        let variants = parse_variant_list(&body_str, source, &name)?;
        if variants.is_empty() {
            return Err(BlastError::Invalid(format!("CREATE TYPE {} in {} declared with zero variants", name, source.display())));
        }
        out.push(ParsedEnum {
            name,
            variants,
            source_file: source.to_path_buf(),
        });
    }
    Ok(out)
}

fn enum_regex() -> BlastResult<Regex> {
    Regex::new(r"(?is)create\s+type\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+as\s+enum\s*\(([^)]*)\)").map_err(BlastError::from)
}

fn parse_variant_list(body: &str, source: &Path, type_name: &str) -> BlastResult<Vec<String>> {
    let mut variants: Vec<String> = Vec::new();
    let bytes = body.as_bytes();
    let mut idx: usize = 0;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch.is_whitespace() || ch == ',' {
            idx += 1;
            continue;
        }
        if ch != '\'' {
            return Err(BlastError::Invalid(format!("CREATE TYPE {} in {}: expected single-quoted variant, found `{}`", type_name, source.display(), ch)));
        }
        idx += 1;
        let start = idx;
        let mut buf = String::new();
        while idx < bytes.len() {
            let cur = bytes[idx] as char;
            if cur == '\'' {
                if idx + 1 < bytes.len() && bytes[idx + 1] as char == '\'' {
                    buf.push('\'');
                    idx += 2;
                    continue;
                }
                break;
            }
            buf.push(cur);
            idx += 1;
        }
        if idx >= bytes.len() {
            return Err(BlastError::Invalid(format!(
                "CREATE TYPE {} in {}: unterminated variant literal starting at byte {}",
                type_name,
                source.display(),
                start
            )));
        }
        idx += 1;
        variants.push(buf);
    }
    Ok(variants)
}

fn strip_line_comments(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    for line in sql.lines() {
        let trimmed = match find_line_comment_start(line) {
            Some(i) => &line[..i],
            None => line,
        };
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

fn find_line_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_single = false;
    while i + 1 < bytes.len() {
        let c = bytes[i] as char;
        if c == '\'' {
            in_single = !in_single;
        } else if !in_single && c == '-' && bytes[i + 1] as char == '-' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;

    fn write_migration(root: &Path, dir: &str, body: &str) {
        let mig = root.join("src/database/migrations").join(dir);
        fs::create_dir_all(&mig).expect("mkdir migration");
        let mut f = fs::File::create(mig.join("up.sql")).expect("create up.sql");
        f.write_all(body.as_bytes()).expect("write up.sql");
    }

    #[test]
    fn parses_simple_create_type() {
        let path = Path::new("/tmp/up.sql");
        let body = "CREATE TYPE user_role AS ENUM ('admin', 'member');";
        let parsed = parse_enums_in_sql(body, path).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "user_role");
        assert_eq!(parsed[0].variants, vec!["admin", "member"]);
    }

    #[test]
    fn case_insensitive_keywords() {
        let path = Path::new("/tmp/up.sql");
        let body = "create Type my_enum As enum ('a','b');";
        let parsed = parse_enums_in_sql(body, path).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "my_enum");
        assert_eq!(parsed[0].variants, vec!["a", "b"]);
    }

    #[test]
    fn multiline_statement_with_trailing_comma_tolerated() {
        let path = Path::new("/tmp/up.sql");
        let body = "CREATE TYPE\n  status\nAS ENUM (\n  'open',\n  'closed',\n  'archived'\n);";
        let parsed = parse_enums_in_sql(body, path).expect("parse");
        assert_eq!(parsed[0].variants, vec!["open", "closed", "archived"]);
    }

    #[test]
    fn ignores_line_comments() {
        let path = Path::new("/tmp/up.sql");
        let body = "-- top comment\nCREATE TYPE x AS ENUM ('a' -- inline\n, 'b');";
        let parsed = parse_enums_in_sql(body, path).expect("parse");
        assert_eq!(parsed[0].variants, vec!["a", "b"]);
    }

    #[test]
    fn doubled_single_quote_escapes() {
        let path = Path::new("/tmp/up.sql");
        let body = "CREATE TYPE x AS ENUM ('a''b', 'c');";
        let parsed = parse_enums_in_sql(body, path).expect("parse");
        assert_eq!(parsed[0].variants, vec!["a'b", "c"]);
    }

    #[test]
    fn multiple_create_types_in_one_file() {
        let path = Path::new("/tmp/up.sql");
        let body = "CREATE TYPE r AS ENUM ('a');\nCREATE TYPE s AS ENUM ('b','c');";
        let parsed = parse_enums_in_sql(body, path).expect("parse");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "r");
        assert_eq!(parsed[1].name, "s");
    }

    #[test]
    fn empty_variant_list_is_error() {
        let path = Path::new("/tmp/up.sql");
        let body = "CREATE TYPE x AS ENUM ();";
        let res = parse_enums_in_sql(body, path);
        assert!(res.is_err());
    }

    #[test]
    fn project_scan_finds_enums_across_migrations() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_migration(root, "2026-01-01-000001_a", "CREATE TYPE user_role AS ENUM ('admin','member');");
        write_migration(root, "2026-01-02-000002_b", "CREATE TYPE post_status AS ENUM ('draft','live','hidden');");

        let report = scan_project_enums(root).expect("scan");
        assert_eq!(report.enums.len(), 2);
        let names: Vec<&str> = report.enums.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"user_role"));
        assert!(names.contains(&"post_status"));
    }

    #[test]
    fn project_scan_records_duplicates() {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path();
        write_migration(root, "2026-01-01-000001_a", "CREATE TYPE x AS ENUM ('a','b');");
        write_migration(root, "2026-01-02-000002_b", "CREATE TYPE x AS ENUM ('c');");

        let report = scan_project_enums(root).expect("scan");
        assert_eq!(report.enums.len(), 1);
        assert_eq!(report.enums[0].variants, vec!["a", "b"]);
        assert_eq!(report.duplicates, vec!["x".to_string()]);
    }

    #[test]
    fn project_scan_handles_missing_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let report = scan_project_enums(tmp.path()).expect("scan");
        assert!(report.enums.is_empty());
        assert!(report.duplicates.is_empty());
    }
}
