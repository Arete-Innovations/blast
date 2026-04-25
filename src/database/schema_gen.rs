use crate::database::connection::mask_url;
use crate::database::migrations::ensure_diesel_postgres;
use crate::progress::ProgressManager;
use crate::logger;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn log_info(msg: &str) {
    if let Err(e) = logger::info(msg) {
        drop(e);
    }
}

fn log_error(msg: &str) {
    if let Err(e) = logger::error(msg) {
        drop(e);
    }
}

fn read_database_url() -> Option<String> {
    let env_content = match fs::read_to_string(".env") {
        Ok(c) => c,
        Err(_e) => return None,
    };
    for line in env_content.lines() {
        if line.starts_with("DATABASE_URL=") {
            return Some(line["DATABASE_URL=".len()..].trim().trim_matches('"').to_string());
        }
    }
    None
}

fn write_schema_file(database_url: &str, schema_file: &str) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message(&format!(
        "Running diesel print-schema with --database-url = {}",
        mask_url(database_url)
    ));

    let output = match Command::new("diesel")
        .args(["print-schema", "--database-url", database_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            progress.error(&format!("Failed to execute diesel command: {}", e));
            return false;
        }
    };

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        progress.error(&format!("Diesel print-schema failed: {}", error));
        return false;
    }

    let schema_str = String::from_utf8_lossy(&output.stdout);

    let mut file = match File::create(schema_file) {
        Ok(f) => f,
        Err(e) => {
            progress.error(&format!("Error creating schema file: {}", e));
            return false;
        }
    };

    if let Err(e) = file.write_all(schema_str.as_bytes()) {
        progress.error(&format!("Error writing schema file: {}", e));
        return false;
    }

    let table_count = schema_str.matches("table!").count();
    progress.success(&format!("Schema generated with {} tables", table_count));
    true
}

fn update_schema_mod_file() {
    let mod_path = "src/database/mod.rs";

    let existing_content = if Path::new(mod_path).exists() {
        match fs::read_to_string(mod_path) {
            Ok(c) => c,
            Err(e) => {
                log_error(&format!("Error reading {}: {}", mod_path, e));
                return;
            }
        }
    } else {
        String::new()
    };

    let other_modules: Vec<String> = existing_content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("pub mod ") && !trimmed.starts_with("pub mod schema")
        })
        .map(|line| line.to_string())
        .collect();

    let mut content = String::from("pub mod schema;\n");
    for module in other_modules {
        content.push_str(&format!("{}\n", module));
    }

    if let Err(e) = fs::write(mod_path, content) {
        log_error(&format!("Error updating schema mod.rs file: {}", e));
    }
}

pub fn generate_schema() -> bool {
    ensure_diesel_postgres();

    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating database schema...");

    if !Path::new("src/database").exists() {
        if let Err(e) = fs::create_dir_all("src/database") {
            progress.error(&format!("Error creating schema directory: {}", e));
            return false;
        }
    }

    let database_url = match read_database_url() {
        Some(url) => url,
        None => {
            progress.error("DATABASE_URL not found in .env file. It is required for schema generation.");
            return false;
        }
    };

    let masked_url = mask_url(&database_url);
    log_info(&format!("Using database URL: {} for schema generation", masked_url));

    if let Err(e) = PgConnection::establish(&database_url) {
        progress.error(&format!(
            "Database connection failed: {}. Is PostgreSQL running?",
            e
        ));
        return false;
    }
    log_info("Connection successful, continuing with schema generation");

    if !write_schema_file(&database_url, "src/database/schema.rs") {
        return false;
    }

    update_schema_mod_file();
    true
}

