//! Tiny string templates the scaffolder still emits OUTSIDE the vendored
//! canonical tree. Keep this file small — anything that lives in the
//! framework itself belongs in `templates/canonical/`, not here.

pub fn env_example(database_url: &str) -> String {
    format!("DATABASE_URL={url}\nBLAST_ENV=dev\nRUST_LOG=info\n", url = database_url,)
}

pub fn env_test_example(database_url: &str) -> String {
    format!("DATABASE_URL={url}\nBLAST_ENV=test\nRUST_LOG=warn\n", url = database_url,)
}
