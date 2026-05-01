use std::{fs, path::PathBuf};

use crate::error::{BlastError, BlastResult};

pub fn write_migration(name: &str, up_sql: &str, down_sql: &str) -> BlastResult<PathBuf> {
    let validated = validate_migration_name(name)?;
    let timestamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S").to_string();
    let dir_name = format!("{}_{}", timestamp, validated);

    let migrations_root = PathBuf::from("src/database/migrations");
    fs::create_dir_all(&migrations_root)?;

    let migration_dir = migrations_root.join(&dir_name);
    if migration_dir.exists() {
        return Err(BlastError::Invalid(format!("migration directory already exists: {}", migration_dir.display())));
    }

    fs::create_dir_all(&migration_dir)?;

    let up_path = migration_dir.join("up.sql");
    let down_path = migration_dir.join("down.sql");

    fs::write(&up_path, up_sql)?;
    fs::write(&down_path, down_sql)?;

    Ok(migration_dir)
}

fn validate_migration_name(name: &str) -> BlastResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(BlastError::Invalid("migration name cannot be empty".to_string()));
    }
    let valid = trimmed.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    let first_char = match trimmed.chars().next() {
        Some(c) => c,
        None => {
            return Err(BlastError::Invalid("migration name cannot be empty".to_string()));
        }
    };
    let starts_letter = first_char.is_ascii_lowercase();
    if !valid || !starts_letter {
        return Err(BlastError::Invalid(format!("migration name '{}' must match snake_case (^[a-z][a-z0-9_]*$)", trimmed)));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf, sync::Mutex};

    use super::*;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that restores the prior cwd on Drop. Critical because
    /// `env::set_current_dir` is process-global, not thread-local, and a
    /// panic inside the test body would otherwise leave the cwd polluted —
    /// subsequent tests would silently operate from the wrong dir.
    struct CwdGuard {
        prev: PathBuf,
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            match env::set_current_dir(&self.prev) {
                Ok(()) => {}
                Err(_restore_err) => {} // allow: Drop can't propagate; next test's CwdGuard chdir overwrites anyway
            }
        }
    }

    fn with_tempdir<F: FnOnce()>(f: F) {
        let _lock = match CWD_LOCK.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = env::current_dir().expect("cwd");
        env::set_current_dir(tmp.path()).expect("chdir tmp");
        let _guard = CwdGuard { prev };
        f();
        // _guard drops here, restoring cwd even if f() panicked.
        // _lock drops next, releasing the mutex.
    }

    #[test]
    fn writes_up_and_down_sql_under_timestamped_dir() {
        with_tempdir(|| {
            let dir = write_migration("create_widgets", "CREATE TABLE widgets ();", "DROP TABLE widgets;").expect("write migration");
            assert!(dir.exists());
            let up = dir.join("up.sql");
            let down = dir.join("down.sql");
            assert_eq!(fs::read_to_string(&up).expect("up"), "CREATE TABLE widgets ();");
            assert_eq!(fs::read_to_string(&down).expect("down"), "DROP TABLE widgets;");

            let dir_name = match dir.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => panic!("no dir name"),
            };
            let parts: Vec<&str> = dir_name.splitn(2, '_').collect();
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[1], "create_widgets");
            let ts = parts[0];
            assert_eq!(ts.len(), 17, "timestamp YYYY-MM-DD-HHMMSS");
        });
    }

    #[test]
    fn rejects_invalid_names() {
        with_tempdir(|| {
            let cases = ["", "Create_X", "1bad", "has space", "has-dash", "BAD"];
            for case in cases {
                let err = write_migration(case, "", "");
                assert!(err.is_err(), "should reject {:?}", case);
            }
        });
    }

    #[test]
    fn accepts_snake_case() {
        with_tempdir(|| {
            let dir = write_migration("alter_users_add_email_v2", "", "").expect("ok");
            assert!(dir.exists());
        });
    }
}
