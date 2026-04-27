use crate::error::BlastResult;
use crate::state::{ListOptions, ResourceState, Verb};
use chrono::{TimeZone, Utc};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct IndexReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
}

pub trait Clock {
    fn now_unix(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> u64 {
        match ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(_clock_err) => 0, // allow: pre-epoch system clocks fall back to zero deterministically
        }
    }
}

pub fn run(
    project_root: &Path,
    resources: &[ResourceState],
    clock: &dyn Clock,
) -> BlastResult<IndexReport> {
    let migrations_dir = canonical_migrations_dir(project_root);
    fs::create_dir_all(&migrations_dir)?;

    let existing = collect_existing_indices(&migrations_dir)?;

    let mut report = IndexReport::default();

    let mut pairs: Vec<(String, String)> = Vec::new();
    for r in resources {
        let table = r.name.as_str().to_string();
        for col in indexable_columns(r) {
            pairs.push((table.clone(), col));
        }
    }
    pairs.sort();
    pairs.dedup();

    let stamp = format_stamp(clock.now_unix());
    let mut counter: u32 = 0;

    for (table, col) in pairs {
        let key = (table.clone(), col.clone());
        match existing.contains(&key) {
            true => {
                report.skipped.push(migration_dir_stub(&migrations_dir, &table, &col));
            }
            false => {
                let dir = migration_dir(&migrations_dir, &stamp, counter, &table, &col);
                fs::create_dir_all(&dir)?;
                fs::write(dir.join("up.sql"), render_up_sql(&table, &col))?;
                fs::write(dir.join("down.sql"), render_down_sql(&table, &col))?;
                report.written.push(dir);
                counter += 1;
            }
        }
    }

    Ok(report)
}

fn canonical_migrations_dir(project_root: &Path) -> PathBuf {
    project_root.join("src").join("database").join("migrations")
}

fn indexable_columns(resource: &ResourceState) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    let verb_state = match resource.verbs.get(&Verb::List) {
        Some(v) => v,
        None => return out,
    };
    let opts: &ListOptions = match verb_state.list_options.as_ref() {
        Some(o) => o,
        None => return out,
    };
    for (c, _kind) in &opts.filterable_columns {
        out.insert(c.as_str().to_string());
    }
    for c in &opts.sortable_columns {
        out.insert(c.as_str().to_string());
    }
    out
}

fn collect_existing_indices(migrations_dir: &Path) -> BlastResult<BTreeSet<(String, String)>> {
    let mut out: BTreeSet<(String, String)> = BTreeSet::new();
    match migrations_dir.is_dir() {
        false => return Ok(out),
        true => {}
    }

    let walker = match fs::read_dir(migrations_dir) {
        Ok(it) => it,
        Err(_read_err) => return Ok(out),
    };

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_entry_err) => continue,
        };
        let path = entry.path();
        match path.is_dir() {
            true => {
                let up_sql = path.join("up.sql");
                match fs::read_to_string(&up_sql) {
                    Ok(body) => {
                        for (table, col) in parse_indices(&body) {
                            out.insert((table, col));
                        }
                    }
                    Err(_read_err) => continue,
                }
            }
            false => continue,
        }
    }

    Ok(out)
}

fn parse_indices(body: &str) -> Vec<(String, String)> {
    let lowered = body.to_ascii_lowercase();
    let mut pairs: Vec<(String, String)> = Vec::new();
    let needle = "create index";
    let mut search_start = 0usize;
    while let Some(rel_idx) = lowered[search_start..].find(needle) {
        let idx = search_start + rel_idx;
        let after = &body[idx + needle.len()..];
        match capture_table_col(after) {
            Some(captured) => pairs.push(captured),
            None => {}
        }
        search_start = idx + needle.len();
    }
    pairs
}

fn capture_table_col(after: &str) -> Option<(String, String)> {
    let lowered = after.to_ascii_lowercase();
    let on_pos = match lowered.find(" on ") {
        Some(p) => p,
        None => return None,
    };
    let after_on = after[on_pos + 4..].trim_start();
    let table_end = match after_on.find(|c: char| c.is_whitespace() || c == '(') {
        Some(idx) => idx,
        None => after_on.len(),
    };
    let table = after_on[..table_end].trim().to_string();
    let rest = &after_on[table_end..];
    let lparen = match rest.find('(') {
        Some(p) => p,
        None => return None,
    };
    let after_lparen = &rest[lparen + 1..];
    let rparen = match after_lparen.find(')') {
        Some(p) => p,
        None => return None,
    };
    let col = after_lparen[..rparen].trim().to_string();
    match col.contains(',') {
        true => None,
        false => Some((table, col)),
    }
}

fn format_stamp(now_unix: u64) -> String {
    let secs = now_unix as i64;
    match Utc.timestamp_opt(secs, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d-%H%M%S").to_string(),
        None => "1970-01-01-000000".to_string(),
    }
}

fn migration_dir_stub(dir: &Path, table: &str, col: &str) -> PathBuf {
    dir.join(format!("0_idx_{table}_{col}"))
}

fn migration_dir(dir: &Path, stamp: &str, counter: u32, table: &str, col: &str) -> PathBuf {
    dir.join(format!("{stamp}{counter:03}_idx_{table}_{col}"))
}

fn render_up_sql(table: &str, col: &str) -> String {
    format!("CREATE INDEX IF NOT EXISTS idx_{table}_{col} ON {table} ({col});\n")
}

fn render_down_sql(table: &str, col: &str) -> String {
    format!("DROP INDEX IF EXISTS idx_{table}_{col};\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::{FieldName, ResourceName, SqlType};
    use crate::state::{
        AuthMode, FieldState, FieldVariant, FilterKind, ListOptions, Verb, VerbState,
    };
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};
    use tempfile::TempDir;

    struct PinnedClock(u64);
    impl Clock for PinnedClock {
        fn now_unix(&self) -> u64 {
            self.0
        }
    }

    fn variants(items: &[FieldVariant]) -> BTreeSet<FieldVariant> {
        items.iter().copied().collect()
    }

    fn sample(table: &str, filterable: &[&str], sortable: &[&str]) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: variants(&[FieldVariant::Db]),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );

        let mut filterable_set: BTreeMap<FieldName, FilterKind> = BTreeMap::new();
        for f in filterable {
            filterable_set.insert(FieldName::new(*f), FilterKind::Eq);
        }
        let mut sortable_set: BTreeSet<FieldName> = BTreeSet::new();
        for s in sortable {
            sortable_set.insert(FieldName::new(*s));
        }
        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: filterable_set,
                    sortable_columns: sortable_set,
                    default_sort: None,
                    max_page_size: None,
                }),
            },
        );
        let mut r = ResourceState::new(ResourceName::new(table));
        r.fields = fields;
        r.verbs = verbs;
        r
    }

    #[test]
    fn emits_one_migration_dir_per_filterable_column() {
        let tmp = TempDir::new().expect("tempdir");
        let r = sample("users", &["email", "active"], &[]);
        let report = run(tmp.path(), &[r], &PinnedClock(1234567890)).expect("emit");
        assert_eq!(report.written.len(), 2);

        for dir in &report.written {
            assert!(dir.is_dir(), "{} should be a directory", dir.display());
            assert!(dir.join("up.sql").is_file(), "missing up.sql in {}", dir.display());
            assert!(dir.join("down.sql").is_file(), "missing down.sql in {}", dir.display());
        }

        let names: Vec<String> = report
            .written
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.iter().any(|n| n.contains("idx_users_email")));
        assert!(names.iter().any(|n| n.contains("idx_users_active")));
    }

    #[test]
    fn emits_into_canonical_migrations_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let r = sample("users", &["email"], &[]);
        let report = run(tmp.path(), &[r], &PinnedClock(1234567890)).expect("emit");
        let written = &report.written[0];
        assert!(written.starts_with(tmp.path().join("src").join("database").join("migrations")));
    }

    #[test]
    fn unions_filterable_and_sortable() {
        let tmp = TempDir::new().expect("tempdir");
        let r = sample("posts", &["title"], &["created_at"]);
        let report = run(tmp.path(), &[r], &PinnedClock(1)).expect("emit");
        assert_eq!(report.written.len(), 2);
    }

    #[test]
    fn deduplicates_overlap() {
        let tmp = TempDir::new().expect("tempdir");
        let r = sample("posts", &["title", "created_at"], &["title", "id"]);
        let report = run(tmp.path(), &[r], &PinnedClock(1)).expect("emit");
        assert_eq!(report.written.len(), 3);
    }

    #[test]
    fn skips_when_existing_migration_already_indexes_pair() {
        let tmp = TempDir::new().expect("tempdir");
        let migrations_dir = canonical_migrations_dir(tmp.path());
        let existing = migrations_dir.join("2026-01-01-000000_seed");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("up.sql"), "CREATE INDEX foo ON users (email);\n").unwrap();
        fs::write(existing.join("down.sql"), "DROP INDEX foo;\n").unwrap();

        let r = sample("users", &["email"], &[]);
        let report = run(tmp.path(), &[r], &PinnedClock(1234567890)).expect("emit");
        assert_eq!(report.written.len(), 0);
        assert_eq!(report.skipped.len(), 1);
    }

    #[test]
    fn rendered_up_sql_creates_index() {
        let body = render_up_sql("users", "email");
        assert!(body.contains("CREATE INDEX IF NOT EXISTS idx_users_email"));
        assert!(body.contains("ON users (email)"));
    }

    #[test]
    fn rendered_down_sql_drops_index() {
        let body = render_down_sql("users", "email");
        assert!(body.contains("DROP INDEX IF EXISTS idx_users_email"));
    }

    #[test]
    fn parses_create_index_with_if_not_exists() {
        let body = "CREATE INDEX IF NOT EXISTS idx_a ON foo (bar);";
        let parsed = parse_indices(body);
        assert_eq!(parsed, vec![("foo".to_string(), "bar".to_string())]);
    }

    #[test]
    fn parser_skips_composite_indices() {
        let body = "CREATE INDEX idx ON foo (a, b);";
        let parsed = parse_indices(body);
        assert!(parsed.is_empty(), "composites are user-authored");
    }

    #[test]
    fn stamp_format_matches_diesel_convention() {
        let stamp = format_stamp(1234567890);
        assert_eq!(stamp, "2009-02-13-233130");
    }
}
