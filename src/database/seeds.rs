use std::{fs, path::Path};

use diesel::{pg::PgConnection, prelude::*};

use crate::{database::connection::establish_connection, progress::ProgressManager};

fn process_seed_files(connection: &mut PgConnection, seed_files: Vec<String>) -> (bool, Vec<String>, Vec<String>) {
    let mut all_succeeded = true;
    let mut successful_seeds = Vec::new();
    let mut failed_seeds = Vec::new();

    for file in seed_files {
        if run_seed_file(connection, &file) {
            successful_seeds.push(file);
        } else {
            failed_seeds.push(file);
            all_succeeded = false;
        }
    }

    (all_succeeded, successful_seeds, failed_seeds)
}

pub fn seed_specific_file(file_name: &str) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message(&format!("Running seed file {}", file_name));

    let mut connection = match establish_connection() {
        Ok(conn) => conn,
        Err(e) => {
            progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
            return false;
        }
    };

    let seed_path = format!("src/database/seeds/{}", file_name);
    if !Path::new(&seed_path).exists() {
        progress.error(&format!("Seed file {} not found", file_name));
        return false;
    }

    let result = run_seed_file(&mut connection, file_name);
    if result {
        progress.success(&format!("Seed file {} executed successfully", file_name));
    } else {
        progress.error(&format!("Failed to execute seed file {}", file_name));
    }

    result
}

pub fn seed() -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Running database seed operations...");

    let mut connection = match establish_connection() {
        Ok(conn) => conn,
        Err(e) => {
            progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
            return false;
        }
    };

    let seed_dir = Path::new("src/database/seeds");

    if !seed_dir.exists() || !seed_dir.is_dir() {
        progress.error("No seeds directory found. Skipping seed operation.");
        return false;
    }

    let seed_files = match fs::read_dir(seed_dir) {
        Ok(entries) => {
            let mut files: Vec<String> = Vec::new();
            for entry_result in entries {
                match entry_result {
                    Ok(entry) => {
                        if entry.path().is_file() {
                            match entry.path().file_name() {
                                Some(name) => files.push(name.to_string_lossy().into_owned()),
                                None => {}
                            }
                        }
                    }
                    Err(e) => {
                        progress.error(&format!("Error reading seed entry: {}", e));
                    }
                }
            }

            if files.is_empty() {
                progress.error("No seed files found. Skipping seed operation.");
                return false;
            }

            let mut sorted_files = files;
            sorted_files.sort();
            sorted_files
        }
        Err(e) => {
            progress.error(&format!("Error reading seed directory: {}. Skipping seed operation.", e));
            return false;
        }
    };

    run_all_seed_files(&mut connection, seed_files)
}

fn run_all_seed_files(connection: &mut PgConnection, seed_files: Vec<String>) -> bool {
    let seed_progress = ProgressManager::new_spinner();
    seed_progress.set_message("Running all seed files...");

    let (all_succeeded, successful_seeds, failed_seeds) = process_seed_files(connection, seed_files);

    if all_succeeded {
        if !successful_seeds.is_empty() {
            seed_progress.success(&format!("Seeded {} files: {}", successful_seeds.len(), successful_seeds.join(", ")));
        } else {
            seed_progress.success("No seed files to run");
        }
    } else {
        if !failed_seeds.is_empty() {
            seed_progress.error(&format!("Failed to seed files: {}", failed_seeds.join(", ")));
        } else {
            seed_progress.error("Some seed files failed to execute");
        }
        return false;
    }

    all_succeeded
}

fn run_seed_file(connection: &mut PgConnection, file_name: &str) -> bool {
    let seed_path = format!("src/database/seeds/{}", file_name);

    let is_interactive = match std::env::var("BLAST_INTERACTIVE") {
        Ok(v) => v == "1",
        Err(e) => {
            drop(e);
            false
        }
    };

    let sql = match fs::read_to_string(&seed_path) {
        Ok(content) => content,
        Err(e) => {
            let error_msg = format!("Error: Unable to read seed file {}: {}", file_name, e);
            if is_interactive {
                if let Err(log_e) = crate::logger::info(&error_msg) {
                    drop(log_e);
                }
            } else if let Err(log_e) = crate::logger::error(&error_msg) {
                drop(log_e);
            }
            return false;
        }
    };

    let mut success = true;
    let statements = split_sql_into_statements(&sql);

    for (i, statement) in statements.iter().enumerate() {
        let trimmed = statement.trim();
        if trimmed.is_empty() {
            continue;
        }

        match diesel::sql_query(trimmed).execute(connection) {
            Ok(_rows) => {
                if is_interactive {
                    if let Err(log_e) = crate::logger::info(&format!("Statement {} executed successfully", i + 1)) {
                        drop(log_e);
                    }
                }
            }
            Err(e) => {
                success = false;
                let error_msg = format!("Error: Failed to execute statement {} in seed file {}: {}", i + 1, file_name, e);
                if is_interactive {
                    if let Err(log_e) = crate::logger::info(&error_msg) {
                        drop(log_e);
                    }
                } else if let Err(log_e) = crate::logger::error(&error_msg) {
                    drop(log_e);
                }
                break;
            }
        }
    }

    success
}

fn split_sql_into_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current_statement = String::new();
    let mut in_string = false;
    let mut in_comment = false;
    let mut chars = sql.chars().peekable();

    loop {
        let Some(c) = chars.next() else { break };
        match c {
            '\'' => {
                if !in_comment && chars.peek() != Some(&'\'') {
                    in_string = !in_string;
                }
                current_statement.push(c);
            }
            '-' => {
                current_statement.push(c);
                if !in_string && chars.peek() == Some(&'-') {
                    in_comment = true;
                    let Some(n) = chars.next() else { continue };
                    current_statement.push(n);
                }
            }
            '\n' => {
                current_statement.push(c);
                if in_comment {
                    in_comment = false;
                }
            }
            ';' => {
                if !in_string && !in_comment {
                    current_statement.push(c);
                    statements.push(current_statement);
                    current_statement = String::new();
                } else {
                    current_statement.push(c);
                }
            }
            other => {
                current_statement.push(other);
            }
        }
    }

    if !current_statement.trim().is_empty() {
        statements.push(current_statement);
    }

    statements
}
