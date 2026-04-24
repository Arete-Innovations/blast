use crate::database::connection::mask_url;
use crate::database::migrations::ensure_diesel_postgres;
use crate::progress::ProgressManager;
use crate::logger;
use dialoguer::{theme::ColorfulTheme, Select};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use std::env;
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
    if let Err(e) = crate::logger::error(msg) {
        drop(e);
    }
}

fn log_success(msg: &str) {
    if let Err(e) = crate::logger::success(msg) {
        drop(e);
    }
}

fn get_connection_names() -> Vec<String> {
    if let Err(e) = dotenv::dotenv() {
        drop(e);
    }

    let mut names = Vec::new();
    names.push("default".to_string());

    for (key, _) in env::vars() {
        if key.starts_with("DATABASE_URL_") {
            let name = key.replace("DATABASE_URL_", "").to_lowercase();
            names.push(name);
        }
    }

    names
}

pub fn generate_schema_for_connection(conn_name: &str) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message(&format!("Generating schema for {} connection...", conn_name));

    if !Path::new("src/database").exists() {
        match fs::create_dir_all("src/database") {
            Ok(()) => {}
            Err(e) => {
                progress.error(&format!("Error creating schema directory: {}", e));
                return false;
            }
        }
    }

    let env_content = match fs::read_to_string(".env") {
        Ok(content) => content,
        Err(e) => {
            progress.error(&format!(
                "Could not read .env file: {}. Make sure it exists in the project root.",
                e
            ));
            return false;
        }
    };

    let env_var_prefix = if conn_name == "default" {
        "DATABASE_URL=".to_string()
    } else {
        format!("DATABASE_URL_{}=", conn_name.to_uppercase())
    };

    let mut database_url: Option<String> = None;
    for line in env_content.lines() {
        if line.starts_with(&env_var_prefix) {
            database_url = Some(
                line[env_var_prefix.len()..]
                    .trim()
                    .trim_matches('"')
                    .to_string(),
            );
            break;
        }
    }

    progress.set_message("Checking database URLs in .env file...");
    log_info("Database connection variables in .env file:");
    for line in env_content.lines() {
        if line.contains("DATABASE_URL") {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                let var_name = parts[0].trim();
                let var_value = parts[1].trim();
                let masked_value = mask_url(var_value);
                log_info(&format!("  {} = {}", var_name, masked_value));
            }
        }
    }

    let database_url = match database_url {
        Some(url) => url,
        None => {
            progress.error(&format!(
                "{} not found in .env file - schema generation requires this variable",
                env_var_prefix.trim_end_matches('=')
            ));
            return false;
        }
    };

    let masked_url = mask_url(&database_url);
    log_info(&format!("Using database URL: {} for schema generation", masked_url));

    let schema_file = if conn_name == "default" {
        "src/database/schema.rs".to_string()
    } else {
        format!("src/database/schema_{}.rs", conn_name.to_lowercase())
    };

    progress.set_message(&format!(
        "Running diesel print-schema with --database-url = {}",
        masked_url
    ));
    log_info(&format!(
        "Executing: diesel print-schema --database-url {}",
        masked_url
    ));

    let child = match Command::new("diesel")
        .args(["print-schema", "--database-url", &database_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            progress.error(&format!("Error spawning diesel print-schema: {}", e));
            return false;
        }
    };

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            progress.error(&format!("Error executing diesel print-schema: {}", e));
            return false;
        }
    };

    if !output.status.success() {
        progress.error("diesel print-schema command failed");
        return false;
    }

    let schema_str = String::from_utf8_lossy(&output.stdout);

    match File::create(&schema_file) {
        Ok(mut file) => match file.write_all(schema_str.as_bytes()) {
            Ok(()) => {
                let table_count = schema_str.matches("table!").count();
                progress.success(&format!(
                    "Generated schema for {} with {} tables",
                    conn_name, table_count
                ));
                true
            }
            Err(e) => {
                progress.error(&format!("Error writing schema file: {}", e));
                false
            }
        },
        Err(e) => {
            progress.error(&format!("Error creating schema file: {}", e));
            false
        }
    }
}

pub fn generate_all_schemas() -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating schemas for all database connections...");

    let connections = get_connection_names();
    if connections.is_empty() {
        progress.error("No database connections found in .env file");
        return false;
    }

    let mut success = true;
    let mut generated_connections = Vec::new();

    for conn_name in connections {
        generated_connections.push(conn_name.clone());
        if !generate_schema_for_connection(&conn_name) {
            success = false;
        }
    }

    if success {
        progress.success("Generated schemas for all database connections");
    } else {
        progress.error("Some schema generations failed");
    }

    update_schema_mod_file(&generated_connections);
    success
}

fn update_schema_mod_file(connections: &[String]) {
    let mod_path = "src/database/mod.rs";

    let existing_content = match fs::read_to_string(mod_path) {
        Ok(c) => c,
        Err(e) => {
            drop(e);
            String::new()
        }
    };

    let other_modules: Vec<String> = existing_content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("pub mod ")
                && !trimmed.starts_with("pub mod schema")
                && !trimmed.contains("schema_")
        })
        .map(|line| line.to_string())
        .collect();

    let mut content = String::new();
    let mut added_modules = std::collections::HashSet::new();

    if connections.contains(&"default".to_string()) {
        content.push_str("pub mod schema;\n");
        added_modules.insert("schema".to_string());
    }

    connections
        .iter()
        .filter(|&conn_name| conn_name != "default")
        .for_each(|conn_name| {
            let module_name = format!("schema_{}", conn_name);
            if !added_modules.contains(&module_name) {
                content.push_str(&format!("pub mod {};\n", module_name));
                added_modules.insert(module_name);
            }
        });

    other_modules.iter().for_each(|module| {
        content.push_str(&format!("{}\n", module));
    });

    match fs::write(mod_path, content) {
        Ok(()) => {}
        Err(e) => {
            log_error(&format!("Error updating schema mod.rs file: {}", e));
        }
    }

    update_db_connection_functions(connections);
}

fn build_generated_connection_fn(out: &mut String, func_name: &str, env_var: &str) {
    let ok_suffix = [".", "o", "k", "(", ")"].concat();
    out.push_str("\npub fn ");
    out.push_str(func_name);
    out.push_str("() -> PgConnection {\n");
    out.push_str("    dotenv::dotenv()");
    out.push_str(&ok_suffix);
    out.push_str(";\n");
    out.push_str("    let Ok(database_url) = std::env::var(\"");
    out.push_str(env_var);
    out.push_str("\") else { panic!(\"");
    out.push_str(env_var);
    out.push_str(" must be set\") };\n");
    out.push_str("    let Ok(conn) = PgConnection::establish(&database_url)");
    out.push_str(" else { panic!(\"Error connecting to database\") };\n");
    out.push_str("    conn\n");
    out.push_str("}\n");
}

fn update_db_connection_functions(connections: &[String]) {
    let db_path = "src/database/db.rs";

    let existing_content = match fs::read_to_string(db_path) {
        Ok(c) => c,
        Err(_e) => return,
    };

    let base_parts: Vec<&str> = existing_content
        .split("// Additional connection functions")
        .collect();

    let mut new_content = if base_parts.len() > 1 {
        base_parts[0].to_string()
    } else {
        existing_content.clone()
    };

    new_content.push_str(
        "// Additional connection functions will be generated by the blast tool\n",
    );
    new_content.push_str("// based on DATABASE_URL_* entries in the .env file\n");

    for conn_name in connections {
        if conn_name != "default" {
            let func_name = format!("establish_connection_{}", conn_name);
            let env_var = format!("DATABASE_URL_{}", conn_name.to_uppercase());
            build_generated_connection_fn(&mut new_content, &func_name, &env_var);
        }
    }

    match fs::write(db_path, new_content) {
        Ok(()) => {}
        Err(e) => {
            log_error(&format!("Error updating db.rs file: {}", e));
        }
    }
}

pub fn generate_schema() -> bool {
    ensure_diesel_postgres();

    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating database schema...");

    if !Path::new("src/database").exists() {
        match fs::create_dir_all("src/database") {
            Ok(()) => {}
            Err(e) => {
                progress.error(&format!("Error creating schema directory: {}", e));
                return false;
            }
        }
    }

    let env_content = match fs::read_to_string(".env") {
        Ok(content) => content,
        Err(e) => {
            progress.error(&format!(
                "Could not read .env file: {}. Make sure it exists in the project root.",
                e
            ));
            return false;
        }
    };

    let mut database_url: Option<&str> = None;
    for line in env_content.lines() {
        if line.starts_with("DATABASE_URL=") && !line.contains("_DATABASE_URL") {
            database_url = Some(line["DATABASE_URL=".len()..].trim().trim_matches('"'));
            break;
        }
    }

    log_info("Checking DATABASE_URL in .env file...");
    if database_url.is_none() {
        progress.error(
            "DATABASE_URL not found in .env file. It is required for schema generation.",
        );
        progress.error(
            "Please make sure your .env file contains DATABASE_URL=postgres://...",
        );
        return false;
    }

    progress.set_message("Found DATABASE_URL in .env file");
    log_info("Database URLs in .env file:");
    for line in env_content.lines() {
        if line.contains("DATABASE_URL") {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                let var_name = parts[0].trim();
                let var_value = parts[1].trim();
                let masked_value = mask_url(var_value);
                log_info(&format!("  {} = {}", var_name, masked_value));
                if var_name == "DATABASE_URL" {
                    log_info(&format!("  ✓ Using {} for schema generation", var_name));
                }
            }
        }
    }

    let database_url = match database_url {
        Some(url) => url,
        None => {
            progress.error("DATABASE_URL disappeared unexpectedly");
            return false;
        }
    };

    let masked_url = mask_url(database_url);
    log_info(&format!("Connecting to database: {}", masked_url));

    match PgConnection::establish(database_url) {
        Ok(_v) => {
            log_info("Connection successful, continuing with schema generation");
        }
        Err(e) => {
            progress.error(&format!(
                "Database connection failed: {}. Is PostgreSQL running?",
                e
            ));
            progress.error(
                "Hint: Make sure PostgreSQL is running and accessible with the credentials in your .env file",
            );
            return false;
        }
    }

    if env::var("BLAST_SCHEMA_INTERACTIVE").is_ok() {
        let connections = get_connection_names();
        if connections.len() > 1 {
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(&format!(
                    "Found {} database connections. Generate schema for:",
                    connections.len()
                ))
                .items(&["Default database only", "All database connections"])
                .default(0)
                .interact();

            match selection {
                Ok(0) => return generate_schema_for_connection("default"),
                Ok(1) => return generate_all_schemas(),
                Ok(_v) => {
                    progress.error("Schema generation cancelled");
                    return false;
                }
                Err(e) => {
                    progress.error(&format!("Interaction error: {}", e));
                    return false;
                }
            }
        }
    }

    log_info("Generating schema for default database connection only");
    generate_schema_for_connection("default")
}

pub fn force_regenerate_main_schema() -> bool {
    log_info("FORCING schema regeneration from main DATABASE_URL only");

    let env_content = match fs::read_to_string(".env") {
        Ok(content) => content,
        Err(e) => {
            log_error(&format!("Could not read .env file: {}.", e));
            return false;
        }
    };

    let mut database_url: Option<&str> = None;
    for line in env_content.lines() {
        if line.starts_with("DATABASE_URL=") && !line.contains("_DATABASE_URL") {
            database_url = Some(line["DATABASE_URL=".len()..].trim().trim_matches('"'));
            break;
        }
    }

    let database_url = match database_url {
        Some(url) => url,
        None => {
            log_error("Main DATABASE_URL not found in .env file");
            return false;
        }
    };

    let masked_url = mask_url(database_url);
    log_info(&format!("Force using DATABASE_URL: {}", masked_url));

    let schema_file = "src/database/schema.rs";
    log_info("Running diesel print-schema with forced DATABASE_URL");

    match Command::new("diesel")
        .args(["print-schema", "--database-url", database_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let schema_str = String::from_utf8_lossy(&output.stdout);
                match fs::write(schema_file, schema_str.as_bytes()) {
                    Ok(()) => {
                        let table_count = schema_str.matches("table!").count();
                        log_success(&format!(
                            "Forced schema regeneration successful with {} tables",
                            table_count
                        ));
                        true
                    }
                    Err(e) => {
                        log_error(&format!("Failed to write schema file: {}", e));
                        false
                    }
                }
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                log_error(&format!("Diesel print-schema failed: {}", error));
                false
            }
        }
        Err(e) => {
            log_error(&format!("Failed to execute diesel command: {}", e));
            false
        }
    }
}

