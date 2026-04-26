use crate::error::{BlastError, BlastResult};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use rand_core06::OsRng;
use std::fs;
use std::path::{Path, PathBuf};

/// Hash a plaintext password with argon2id and a random salt.
/// Returns a PHC string starting with `$argon2id$`.
pub fn hash_password(plaintext: &str) -> BlastResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(plaintext.as_bytes(), &salt)
        .map_err(|e| BlastError::Project(format!("argon2 hashing failed: {e}")))?;
    Ok(hash.to_string())
}

/// Build the full up.sql text for migration 0001_users_and_sessions.
/// `admin_password_hash` must be a valid argon2 PHC string.
pub fn up_sql(admin_password_hash: &str) -> String {
    format!(
        r#"-- 0001 baseline auth schema
-- Creates users + sessions tables and seeds the default admin account.

CREATE TABLE users (
    id           BIGSERIAL PRIMARY KEY,
    email        TEXT      NOT NULL UNIQUE,
    password_hash TEXT     NOT NULL,
    role         TEXT      NOT NULL DEFAULT 'user',
    created_at   BIGINT   NOT NULL DEFAULT extract(epoch from NOW())::bigint,
    updated_at   BIGINT   NOT NULL DEFAULT extract(epoch from NOW())::bigint,
    deleted_at   BIGINT   NULL
);

CREATE INDEX users_email_idx ON users (email);

CREATE TABLE sessions (
    id         BIGSERIAL PRIMARY KEY,
    user_id    BIGINT    NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token      TEXT      NOT NULL UNIQUE,
    expires_at BIGINT    NOT NULL,
    created_at BIGINT    NOT NULL DEFAULT extract(epoch from NOW())::bigint
);

CREATE INDEX sessions_token_idx   ON sessions (token);
CREATE INDEX sessions_user_id_idx ON sessions (user_id);

INSERT INTO users (email, password_hash, role)
VALUES ('admin', '{hash}', 'admin');
"#,
        hash = admin_password_hash,
    )
}

/// Build the down.sql text — drops sessions (FK child) before users (FK parent).
pub fn down_sql() -> &'static str {
    r#"-- 0001 baseline auth schema rollback
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS users;
"#
}

/// Emit `migrations/0001_users_and_sessions/{up.sql,down.sql}` into `migrations_dir`.
/// Returns the two paths written.
pub fn emit(migrations_dir: &Path) -> BlastResult<Vec<PathBuf>> {
    let dir = migrations_dir.join("0001_users_and_sessions");
    fs::create_dir_all(&dir)?;

    let admin_hash = hash_password("admin")?;

    let up_path = dir.join("up.sql");
    let down_path = dir.join("down.sql");

    fs::write(&up_path, up_sql(&admin_hash))?;
    fs::write(&down_path, down_sql())?;

    Ok(vec![up_path, down_path])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn hash_password_produces_argon2id_phc() {
        let hash = hash_password("admin").expect("hash");
        assert!(
            hash.starts_with("$argon2id$"),
            "expected PHC string starting with $argon2id$, got: {hash}"
        );
    }

    #[test]
    fn hash_password_uses_random_salt() {
        let h1 = hash_password("admin").expect("h1");
        let h2 = hash_password("admin").expect("h2");
        assert_ne!(h1, h2, "two hashes of the same password must differ (random salt)");
    }

    #[test]
    fn up_sql_contains_both_create_tables_and_insert() {
        let fake_hash = "$argon2id$v=19$m=19456,t=2,p=1$fake_salt$fake_hash_output";
        let sql = up_sql(fake_hash);
        assert!(sql.contains("CREATE TABLE users"), "missing CREATE TABLE users");
        assert!(sql.contains("CREATE TABLE sessions"), "missing CREATE TABLE sessions");
        assert!(
            sql.contains(&format!("'{fake_hash}'")),
            "hash not substituted into INSERT"
        );
        assert!(sql.contains("INSERT INTO users"), "missing INSERT");
    }

    #[test]
    fn up_sql_has_correct_column_definitions() {
        let sql = up_sql("$argon2id$placeholder");
        // users columns
        assert!(sql.contains("deleted_at   BIGINT   NULL"), "missing soft-delete column");
        assert!(sql.contains("REFERENCES users(id) ON DELETE CASCADE"), "missing FK cascade");
    }

    #[test]
    fn down_sql_drops_sessions_before_users() {
        let sql = down_sql();
        let sessions_pos = sql.find("sessions").expect("sessions in down.sql");
        let users_pos = sql.find("users").expect("users in down.sql");
        assert!(
            sessions_pos < users_pos,
            "down.sql must drop sessions (FK child) before users"
        );
    }

    #[test]
    fn emit_writes_both_files_at_correct_paths() {
        let tmp = tempdir().expect("tempdir");
        let migrations_dir = tmp.path().join("migrations");
        fs::create_dir_all(&migrations_dir).expect("create migrations dir");

        let written = emit(&migrations_dir).expect("emit");
        assert_eq!(written.len(), 2, "emit should return 2 paths");

        let up = migrations_dir
            .join("0001_users_and_sessions")
            .join("up.sql");
        let down = migrations_dir
            .join("0001_users_and_sessions")
            .join("down.sql");

        assert!(up.is_file(), "up.sql not found at {}", up.display());
        assert!(down.is_file(), "down.sql not found at {}", down.display());

        let up_body = fs::read_to_string(&up).expect("read up.sql");
        assert!(
            up_body.contains("CREATE TABLE users"),
            "up.sql missing CREATE TABLE users"
        );
        assert!(
            up_body.contains("CREATE TABLE sessions"),
            "up.sql missing CREATE TABLE sessions"
        );
        assert!(
            up_body.contains("$argon2id$"),
            "up.sql missing argon2id hash"
        );
    }
}
