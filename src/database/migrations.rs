use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::{database::connection::establish_connection, logger, progress::ProgressManager};

pub fn ensure_diesel_postgres() {
    let mut dep_manager = crate::dependencies::DependencyManager::new();
    match dep_manager.ensure_diesel_with_postgres_features() {
        Ok(()) => {}
        Err(e) => {
            if let Err(log_e) = logger::debug(&format!("Diesel PostgreSQL feature check: {}", e)) {
                drop(log_e);
            }
        }
    }
}

fn handle_diesel_output(output: &std::process::Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let is_interactive = match std::env::var("BLAST_INTERACTIVE") {
        Ok(v) => v == "1",
        Err(e) => {
            drop(e);
            false
        }
    };

    let log_fn = |line: &str, success: bool| {
        let prefix = if success { "\x1b[32m✔\x1b[0m" } else { "\x1b[31m✖\x1b[0m" };
        let formatted_line = format!("{} {}", prefix, line);
        if is_interactive {
            if let Err(e) = crate::logger::info(&formatted_line) {
                drop(e);
            }
        } else if let Err(e) = crate::logger::log(crate::logger::LogLevel::Info, &formatted_line) {
            drop(e);
        }
    };

    stdout.lines().for_each(|line| log_fn(line, true));

    let has_error = stderr
        .lines()
        .map(|line| {
            log_fn(line, false);
            true
        })
        .next()
        .is_some();

    !has_error
}

fn run_diesel_migration(args: &[&str], progress_msg: &str) -> bool {
    ensure_diesel_postgres();

    let progress = ProgressManager::new_spinner();
    progress.set_message(progress_msg);

    match establish_connection() {
        Ok(_v) => {}
        Err(e) => {
            progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
            progress.error("Hint: Make sure PostgreSQL is running and accessible with the credentials in your .env file");
            return false;
        }
    }

    let output = match Command::new("diesel").args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
        Ok(output) => output,
        Err(e) => {
            progress.error(&format!("Failed to execute command: {}", e));
            return false;
        }
    };

    handle_diesel_output(&output)
}

pub fn rollback_all() -> bool {
    run_diesel_migration(&["migration", "revert", "--all"], "Rolling back all migrations...")
}

pub fn migrate() -> bool {
    if !Path::new("src/database/migrations").exists() {
        let progress = ProgressManager::new_spinner();
        progress.set_message("Checking migrations directory...");
        progress.error("No migrations directory found. Skipping migration operation.");
        return false;
    }

    ensure_diesel_postgres();

    let is_verbose = logger::is_verbose();

    let progress = ProgressManager::new_spinner();
    progress.set_message("Running database migrations...");

    match establish_connection() {
        Ok(_v) => {}
        Err(e) => {
            progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
            progress.error("Hint: Make sure PostgreSQL is running and accessible with the credentials in your .env file");
            return false;
        }
    }

    let output = match Command::new("diesel").args(["migration", "run"]).stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
        Ok(output) => output,
        Err(e) => {
            progress.error(&format!("Error executing diesel migration run: {}", e));
            return false;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let migrations: Vec<String> = stdout
        .lines()
        .filter(|line| line.contains("Running migration"))
        .filter_map(|line| line.split("Running migration").nth(1).map(|name| name.trim().to_string()))
        .collect();

    let has_output = stdout.lines().next().is_some();
    let errors: Vec<String> = stderr.lines().map(|line| line.trim().to_string()).collect();
    let has_errors = !errors.is_empty();

    if is_verbose && has_errors {
        eprintln!("\n\x1b[1;33m===== VERBOSE MIGRATION DIAGNOSTICS =====\x1b[0m");

        for error in &errors {
            eprintln!("\x1b[1;31mERROR:\x1b[0m {}", error);

            if error.contains("Invalid migration directory") {
                eprintln!("\x1b[1;33m⚠️  Migration directory structure issue detected.\x1b[0m");
                eprintln!("ℹ️  Migration directories must follow the pattern:");
                eprintln!("  - <timestamp>_<name_of_migration>");
                eprintln!("  - Example: 20250101000001_create_users");
                eprintln!("  - Each directory must contain up.sql and optionally down.sql");

                match std::fs::read_dir("src/database/migrations") {
                    Ok(entries) => {
                        eprintln!("\nℹ️  Current migration directories:");
                        for entry in entries.filter_map(Result::ok).filter(|e| e.path().is_dir()) {
                            let name = entry.file_name();
                            let name_str = name.to_string_lossy();

                            let parts: Vec<&str> = name_str.split('_').collect();
                            let name_valid = parts.len() >= 2 && parts[0].chars().all(|c| c.is_ascii_digit());

                            let up_sql_path = entry.path().join("up.sql");
                            let has_up_sql = up_sql_path.exists();

                            if name_valid && has_up_sql {
                                eprintln!("  \x1b[32m✓\x1b[0m {}", name_str);
                            } else {
                                eprintln!("  \x1b[31m✗\x1b[0m {} \x1b[1;33m<-- ISSUE\x1b[0m", name_str);

                                if !has_up_sql {
                                    eprintln!("    \x1b[31m⚠️ Missing up.sql file\x1b[0m");
                                }

                                if !name_valid {
                                    if parts.len() < 2 {
                                        eprintln!("    \x1b[31m⚠️ Invalid format: should be <timestamp>_<name>\x1b[0m");
                                    } else if !parts[0].chars().all(|c| c.is_ascii_digit()) {
                                        eprintln!("    \x1b[31m⚠️ First part should be a numeric timestamp\x1b[0m");
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  Could not list migration directories: {}", e);
                    }
                }
            } else if error.contains("No such file or directory") {
                eprintln!("\x1b[1;33m⚠️  File or directory missing.\x1b[0m");
                eprintln!("ℹ️  Check that all migration directories contain up.sql files.");
            } else if error.contains("permission denied") {
                eprintln!("\x1b[1;33m⚠️  Permission issue detected.\x1b[0m");
                eprintln!("ℹ️  Make sure you have proper permissions for the migration directories.");
            }
        }

        eprintln!("\x1b[1;33m===== END DIAGNOSTICS =====\x1b[0m\n");
    }

    match (has_output, has_errors, migrations.is_empty()) {
        (false, false, _) => progress.success("No migrations to run"),
        (_, false, false) => progress.success(&format!("Ran {} migrations: {}", migrations.len(), migrations.join(", "))),
        (_, false, true) => progress.success("Migrations completed successfully"),
        (_, true, _) => {
            if !errors.is_empty() {
                if is_verbose {
                    progress.error("Migration errors: See detailed diagnostics above");
                    eprintln!("\x1b[1;33mCheck the VERBOSE MIGRATION DIAGNOSTICS section above for detailed error information.\x1b[0m");
                } else {
                    progress.error(&format!("Migration errors: {}. Use -v for more details.", errors.join(", ")));
                }
            } else {
                progress.error("Some migrations failed");
            }
            return false;
        }
    }

    true
}
