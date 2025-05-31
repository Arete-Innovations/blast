use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::configs::Config;
use crate::logger;

// Function to run migrations from a spark plugin
fn run_spark_migration(migration_path: &PathBuf) -> Result<(), String> {
    use dotenv::dotenv;
    use std::collections::HashMap;
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Stdio};

    // Load environment variables from .env file to ensure they're available to migrations
    dotenv().ok();

    // Check if verbose mode is enabled using the logger's built-in mechanism
    // This handles both the -v flag and the BLAST_VERBOSE environment variable
    // Declare it as a function that wraps the logger's verbosity check
    let is_verbose = || {
        // Access the verbosity state from the logger
        // This effectively uses the same check that logger::info uses internally
        env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1" || env::var("BLAST_INTERACTIVE").is_ok()
    };

    if is_verbose() {
        logger::info("VERBOSE MODE ENABLED - Showing detailed migration debugging info")?;
    }

    logger::info(&format!("Running migrations from path: {}", migration_path.display()))?;

    // Check if migrations directory exists
    if !migration_path.exists() || !migration_path.is_dir() {
        return Err(format!("Migration path does not exist: {}", migration_path.display()));
    }

    // Create a progress tracker
    let mut progress = logger::create_progress(None);
    progress.set_message(&format!("Running migrations from: {}", migration_path.display()));

    // Determine which spark this migration belongs to
    // First determine the spark directory by working backwards from migration_path
    let mut spark_dir = None;
    let mut current_path = migration_path.clone();

    // Keep going up directories until we find a manifest.toml file or hit root
    while current_path.parent().is_some() {
        current_path = match current_path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => break,
        };

        let manifest_path = current_path.join("manifest.toml");
        if manifest_path.exists() {
            spark_dir = Some(current_path);
            break;
        }
    }

    // Get the spark name - first try to read from manifest.toml
    let spark_name = if let Some(dir) = spark_dir {
        let manifest_path = dir.join("manifest.toml");
        if manifest_path.exists() {
            match fs::read_to_string(&manifest_path) {
                Ok(content) => {
                    // Simple parsing to extract spark name from manifest.toml
                    let name_line = content.lines().find(|line| line.trim().starts_with("name"));

                    if let Some(line) = name_line {
                        // Extract the value from name = "value"
                        let parts: Vec<&str> = line.split('=').collect();
                        if parts.len() > 1 {
                            // Extract the value and remove quotes and whitespace
                            let value = parts[1].trim().trim_matches('"').trim_matches('\'');
                            value.to_uppercase()
                        } else {
                            // If we can't parse it properly, fall back to directory name
                            dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_uppercase()
                        }
                    } else {
                        // If name not found in manifest, use directory name
                        dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_uppercase()
                    }
                }
                Err(_) => {
                    // If can't read manifest, fall back to directory name
                    dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_uppercase()
                }
            }
        } else {
            // If no manifest.toml, fall back to directory name
            dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_uppercase()
        }
    } else {
        // If we can't find a manifest.toml, fall back to the old path component method
        migration_path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .find(|&s| s != "migrations" && s != "initial" && !s.starts_with("."))
            .unwrap_or("unknown")
            .to_uppercase()
    };

    logger::info(&format!("Looking for database URL for spark: {}", spark_name))?;

    // Verbose debugging: List all the path components for easier diagnosis
    if is_verbose() {
        logger::info("Path component analysis:")?;
        for component in migration_path.components() {
            if let Some(comp_str) = component.as_os_str().to_str() {
                logger::info(&format!("  - Component: {}", comp_str))?;
            }
        }

        // List migration directory contents
        if migration_path.exists() && migration_path.is_dir() {
            logger::info(&format!("Contents of migration directory: {}", migration_path.display()))?;
            match fs::read_dir(migration_path) {
                Ok(entries) => {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let path = entry.path();
                            let file_type = if path.is_dir() { "directory" } else { "file" };
                            logger::info(&format!("  - {} ({})", path.display(), file_type))?;

                            // If it's a directory, also list its contents (for versioned migrations)
                            if path.is_dir() {
                                match fs::read_dir(&path) {
                                    Ok(sub_entries) => {
                                        for sub_entry in sub_entries {
                                            if let Ok(sub_entry) = sub_entry {
                                                logger::info(&format!("    - {}", sub_entry.path().display()))?;
                                            }
                                        }
                                    }
                                    Err(e) => logger::warning(&format!("Failed to read directory {}: {}", path.display(), e))?,
                                }
                            }
                        }
                    }
                }
                Err(e) => logger::warning(&format!("Failed to read migration directory: {}", e))?,
            }
        }

        // List all environment variables
        logger::info("Environment variables:")?;
        for (key, value) in env::vars() {
            if key.contains("DATABASE") || key.contains(spark_name.as_str()) {
                // Mask the actual connection string for security
                let masked_value = if value.contains("://") {
                    let parts: Vec<&str> = value.splitn(2, "://").collect();
                    if parts.len() == 2 {
                        format!("{}://<masked>", parts[0])
                    } else {
                        "<masked>".to_string()
                    }
                } else {
                    "<masked>".to_string()
                };
                logger::info(&format!("  - {}={}", key, masked_value))?;
            }
        }
    }

    // First try the spark-specific database URL (SPARKNAME_DATABASE_URL)
    let spark_db_var = format!("{}_DATABASE_URL", spark_name);

    // Create a map of environment variables with DATABASE_URL set properly
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();

    // IMPORTANT: This override of DATABASE_URL should ONLY affect operations in this function
    // and not impact the main schema generation or other database operations

    // Check for spark-specific database URL
    if let Ok(url) = std::env::var(&spark_db_var) {
        logger::info(&format!("Using spark-specific database URL from {}", spark_db_var))?;
        logger::info("NOTE: This only affects THIS spark migration and not the main schema generation")?;

        // We don't need to store the original URL anymore since we use explicit parameters

        // Temporarily override DATABASE_URL just for this operation
        env_vars.insert("DATABASE_URL".to_string(), url.clone());

        // Also keep a direct reference we can use with --database-url
        env_vars.insert("_SPARK_DIRECT_DB_URL".to_string(), url);
    } else {
        logger::info("No spark-specific DATABASE_URL found, using default")?;
        // Keep the default DATABASE_URL from the environment
    }

    // Step 1: Run diesel setup to ensure migrations table exists
    logger::info("Running diesel setup to prepare database")?;

    // In verbose mode, also examine the diesel.toml file if it exists
    if is_verbose() {
        let diesel_toml_path = Path::new("diesel.toml");
        if diesel_toml_path.exists() {
            match fs::read_to_string(diesel_toml_path) {
                Ok(content) => {
                    logger::info("diesel.toml content:")?;
                    for line in content.lines() {
                        logger::info(&format!("  {}", line))?;
                    }
                }
                Err(e) => logger::warning(&format!("Failed to read diesel.toml: {}", e))?,
            }
        } else {
            logger::info("diesel.toml file not found in current directory")?;
        }

        // Also check if the up.sql file exists and show its content
        let up_sql_path = migration_path.join("up.sql");
        if up_sql_path.exists() {
            match fs::read_to_string(&up_sql_path) {
                Ok(content) => {
                    logger::info(&format!("up.sql content from {}", up_sql_path.display()))?;
                    for (i, line) in content.lines().enumerate() {
                        logger::info(&format!("  {}. {}", i + 1, line))?;
                    }
                }
                Err(e) => logger::warning(&format!("Failed to read up.sql: {}", e))?,
            }
        }
    }

    // Get database URL for setup
    let setup_db_url = if let Some(direct_url) = env_vars.get("_SPARK_DIRECT_DB_URL") {
        direct_url.clone()
    } else if let Some(url) = env_vars.get("DATABASE_URL") {
        url.clone()
    } else {
        return Err("No database URL available for setup".to_string());
    };

    // Always use the explicit --database-url parameter
    let setup_output = Command::new("diesel")
        .args(["setup", "--migration-dir", &migration_path.to_string_lossy(), "--database-url", &setup_db_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match setup_output {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !output.status.success() && !stderr.contains("already exists") {
                logger::warning(&format!("Diesel setup warning: {}", stderr))?;
            } else {
                logger::info("Diesel setup completed")?;
            }
        }
        Err(e) => logger::warning(&format!("Failed to run diesel setup: {}", e))?,
    }

    // Step 2: First try running the migration normally
    logger::info("Running diesel migration")?;

    if is_verbose() {
        // In verbose mode, display the exact diesel command and environment
        let cmd_str = format!("diesel migration run --migration-dir {}", migration_path.display());
        logger::info(&format!("Running command: {}", cmd_str))?;

        // Check if the DATABASE_URL is set correctly
        if let Some(db_url) = env_vars.get("DATABASE_URL") {
            let masked_url = if db_url.contains("://") {
                let parts: Vec<&str> = db_url.splitn(2, "://").collect();
                if parts.len() == 2 {
                    format!("{}://<masked>", parts[0])
                } else {
                    "<masked>".to_string()
                }
            } else {
                "<masked>".to_string()
            };
            logger::info(&format!("Using DATABASE_URL={}", masked_url))?;
        } else {
            logger::warning("DATABASE_URL is not set in environment!")?;
        }

        // Check for diesel binary
        match Command::new("which").arg("diesel").output() {
            Ok(output) => {
                if output.status.success() {
                    let diesel_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    logger::info(&format!("Diesel binary found at: {}", diesel_path))?;
                } else {
                    logger::warning("Diesel binary not found in PATH!")?;
                }
            }
            Err(e) => logger::warning(&format!("Failed to check for diesel binary: {}", e))?,
        }
    }

    // Get the direct database URL to use - prefer the explicit spark URL if available
    let db_url = if let Some(direct_url) = env_vars.get("_SPARK_DIRECT_DB_URL") {
        direct_url.clone()
    } else if let Some(url) = env_vars.get("DATABASE_URL") {
        url.clone()
    } else {
        return Err("No database URL available for running migrations".to_string());
    };

    // Always use --database-url to ensure we're targeting the correct database
    let output = Command::new("diesel")
        .args(["migration", "run", "--migration-dir", &migration_path.to_string_lossy(), "--database-url", &db_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to execute diesel migration: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Log the output for debugging
    logger::info(&format!("Diesel migration stdout: {}", stdout))?;
    if !stderr.is_empty() {
        logger::info(&format!("Diesel migration stderr: {}", stderr))?;
    }

    if is_verbose() {
        // In verbose mode, add extra diagnostic information
        logger::info(&format!("Diesel command exit status: {}", output.status))?;
        logger::info(&format!("Diesel command success: {}", output.status.success()))?;
        if let Some(code) = output.status.code() {
            logger::info(&format!("Diesel command exit code: {}", code))?;
        }
    }

    if !output.status.success() {
        return Err(format!("Migration failed: {}", stderr));
    }

    // Extract migrations from stdout
    let migrations: Vec<String> = stdout
        .lines()
        .filter(|line| line.contains("Running migration"))
        .filter_map(|line| line.split("Running migration").nth(1).map(|name| name.trim().to_string()))
        .collect();

    if !migrations.is_empty() {
        progress.success(&format!("Ran {} migrations: {}", migrations.len(), migrations.join(", ")));
        return Ok(());
    }

    // Step 3: If no migrations were run but command succeeded, try forcing the migration
    // Check if this is a direct migration with up.sql/down.sql
    let up_sql_path = migration_path.join("up.sql");
    let is_direct_migration = up_sql_path.exists();

    if is_direct_migration {
        // This is a direct migration with up.sql/down.sql but diesel didn't run it
        // Try with redo to force re-run
        logger::warning("No migrations run initially - attempting migration redo")?;

        // Get database URL for redo
        let redo_db_url = if let Some(direct_url) = env_vars.get("_SPARK_DIRECT_DB_URL") {
            direct_url.clone()
        } else if let Some(url) = env_vars.get("DATABASE_URL") {
            url.clone()
        } else {
            return Err("No database URL available for migration redo".to_string());
        };

        // Always use the explicit --database-url parameter
        let redo_output = Command::new("diesel")
            .args(["migration", "redo", "--migration-dir", &migration_path.to_string_lossy(), "--database-url", &redo_db_url])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        match redo_output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                logger::info(&format!("Diesel redo stdout: {}", stdout))?;
                if !stderr.is_empty() {
                    logger::info(&format!("Diesel redo stderr: {}", stderr))?;
                }

                if output.status.success() && (stdout.contains("Rolling back") || stdout.contains("Running")) {
                    progress.success("Successfully ran migration via redo");
                    return Ok(());
                } else {
                    logger::warning("Migration redo didn't run anything either")?;
                }
            }
            Err(e) => logger::warning(&format!("Failed to run migration redo: {}", e))?,
        }

        // If redo didn't work, try directly executing the SQL with psql
        logger::warning("Attempting to directly execute migration SQL with psql")?;

        // Read the up.sql file
        let up_sql_content = match fs::read_to_string(&up_sql_path) {
            Ok(content) => content,
            Err(e) => {
                logger::warning(&format!("Failed to read up.sql file: {}", e))?;
                return Ok(());
            }
        };

        // Get the database URL for direct SQL execution - use the spark-specific URL
        let db_url = if let Some(direct_url) = env_vars.get("_SPARK_DIRECT_DB_URL") {
            direct_url.clone()
        } else if let Some(url) = env_vars.get("DATABASE_URL") {
            url.clone()
        } else {
            logger::warning("No database URL available for direct SQL execution")?;
            return Ok(());
        };

        // Write SQL to a temporary file
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let temp_sql_path = format!("/tmp/direct_migration_{}.sql", timestamp);

        if let Err(e) = fs::write(&temp_sql_path, &up_sql_content) {
            logger::warning(&format!("Failed to write temporary SQL file: {}", e))?;
            return Ok(());
        }

        // Execute the SQL directly with psql
        logger::info(&format!("Executing SQL with psql from: {}", temp_sql_path))?;

        let psql_output = Command::new("psql").arg(&db_url).arg("-f").arg(&temp_sql_path).stdout(Stdio::piped()).stderr(Stdio::piped()).output();

        // Clean up temp file no matter what
        let _ = fs::remove_file(&temp_sql_path);

        match psql_output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                logger::info(&format!("Direct psql stdout: {}", stdout))?;
                if !stderr.is_empty() {
                    logger::info(&format!("Direct psql stderr: {}", stderr))?;
                }

                if output.status.success() {
                    // Update diesel_schema_migrations table to make diesel aware of this migration
                    logger::info("Migration SQL executed successfully, updating diesel_schema_migrations table")?;

                    // Get the migration version name (usually the directory name)
                    let version = migration_path.file_name().and_then(|n| n.to_str()).unwrap_or("initial");

                    // Write SQL to insert the migration record
                    let migration_record_sql = format!("INSERT INTO __diesel_schema_migrations (version, run_on) VALUES ('{}', NOW()) ON CONFLICT DO NOTHING;", version);

                    let record_sql_path = format!("/tmp/update_migrations_{}.sql", timestamp);
                    if let Err(e) = fs::write(&record_sql_path, &migration_record_sql) {
                        logger::warning(&format!("Failed to write migration record SQL file: {}", e))?;
                    } else {
                        let update_output = Command::new("psql").arg(&db_url).arg("-f").arg(&record_sql_path).stdout(Stdio::piped()).stderr(Stdio::piped()).output();

                        // Clean up temp file
                        let _ = fs::remove_file(&record_sql_path);

                        match update_output {
                            Ok(update_out) => {
                                let update_stderr = String::from_utf8_lossy(&update_out.stderr);
                                if !update_stderr.is_empty() && !update_stderr.contains("NOTICE") {
                                    logger::warning(&format!("Failed to update migrations table: {}", update_stderr))?;
                                } else {
                                    logger::info("Successfully updated diesel_schema_migrations table")?;
                                }
                            }
                            Err(e) => logger::warning(&format!("Failed to execute migration record update: {}", e))?,
                        }
                    }

                    progress.success("Successfully executed migration SQL directly");
                    return Ok(());
                } else {
                    logger::warning(&format!("Direct SQL execution failed: {}", stderr))?;
                }
            }
            Err(e) => logger::warning(&format!("Failed to execute direct SQL: {}", e))?,
        }
    }

    // We tried everything but nothing ran - assume it's already migrated
    let _ = progress.warning("Diesel did not run any migrations - this likely means they are already applied")?;
    progress.success("No new migrations to run");

    // In verbose mode, check the database schema to see if tables were actually created
    if is_verbose() {
        logger::info("Checking database schema to verify if tables exist...")?;

        // Get database URL for schema check - use the spark-specific URL
        let db_url = if let Some(direct_url) = env_vars.get("_SPARK_DIRECT_DB_URL") {
            direct_url.clone()
        } else if let Some(url) = env_vars.get("DATABASE_URL") {
            url.clone()
        } else {
            match env::var("DATABASE_URL") {
                Ok(url) => url,
                Err(_) => {
                    logger::warning("DATABASE_URL not found for schema check")?;
                    String::new()
                }
            }
        };

        // Always use the explicit database URL parameter
        let schema_output = if !db_url.is_empty() {
            Command::new("diesel").args(["print-schema", "--database-url", &db_url]).stdout(Stdio::piped()).stderr(Stdio::piped()).output()
        } else {
            logger::warning("No database URL available for schema check")?;
            return Ok(());
        };

        match schema_output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                logger::info("Database schema:")?;
                if stdout.trim().is_empty() {
                    logger::warning("No tables found in database schema!")?;
                } else {
                    for line in stdout.lines() {
                        logger::info(&format!("  {}", line))?;
                    }
                }

                if !stderr.is_empty() {
                    logger::warning(&format!("Schema check stderr: {}", stderr))?;
                }
            }
            Err(e) => logger::warning(&format!("Failed to check database schema: {}", e))?,
        }

        // Also try to directly query the database to check if the tables exist
        // Use psql to check if the table exists (only works with PostgreSQL)
        if let Some(db_url) = env_vars.get("DATABASE_URL") {
            if db_url.starts_with("postgres") {
                // Extract table names from up.sql
                let up_sql_path = migration_path.join("up.sql");
                if up_sql_path.exists() {
                    if let Ok(content) = fs::read_to_string(&up_sql_path) {
                        // Simple regex-like parsing to extract table names from CREATE TABLE statements
                        let table_names: Vec<String> = content
                            .lines()
                            .filter(|line| line.to_lowercase().contains("create table") && !line.trim().starts_with("--"))
                            .filter_map(|line| {
                                // Extract table name from "CREATE TABLE [IF NOT EXISTS] table_name"
                                let lower_line = line.to_lowercase();
                                let table_parts = if lower_line.contains("if not exists") {
                                    lower_line.split("if not exists").nth(1)
                                } else {
                                    lower_line.split("table").nth(1)
                                };

                                if let Some(part) = table_parts {
                                    // Remove everything after first ( or space
                                    let end_idx = part.find('(').or_else(|| part.find(' ')).unwrap_or(part.len());
                                    Some(part[..end_idx].trim().to_string())
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if !table_names.is_empty() {
                            logger::info(&format!("Checking for tables found in up.sql: {}", table_names.join(", ")))?;

                            // Create temporary SQL file to query tables
                            let temp_sql = "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public';";
                            let temp_sql_path = Path::new("/tmp/check_tables.sql");
                            if let Err(e) = fs::write(temp_sql_path, temp_sql) {
                                logger::warning(&format!("Failed to write temporary SQL file: {}", e))?;
                            } else {
                                // Run psql to check tables
                                let psql_output = Command::new("psql").arg(db_url).arg("-f").arg(temp_sql_path).stdout(Stdio::piped()).stderr(Stdio::piped()).output();

                                match psql_output {
                                    Ok(output) => {
                                        let stdout = String::from_utf8_lossy(&output.stdout);
                                        logger::info("Database tables:")?;
                                        for line in stdout.lines() {
                                            logger::info(&format!("  {}", line))?;
                                        }
                                    }
                                    Err(e) => logger::warning(&format!("Failed to query database tables: {}", e))?,
                                }

                                // Clean up temp file
                                let _ = fs::remove_file(temp_sql_path);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// Function to update or add sparks in Catalyst.toml file
pub fn update_sparks_toml(spark_name: &str, repo_url: &str) -> Result<bool, String> {
    // Update the Catalyst.toml file to add the spark
    let config_path = Path::new("Catalyst.toml");
    if !config_path.exists() {
        return Err(format!("Catalyst.toml not found in the current directory"));
    }

    // Read and parse the TOML
    let toml_content = fs::read_to_string(config_path).map_err(|e| format!("Failed to read Catalyst.toml: {}", e))?;

    // Parse the content as a document to preserve formatting and comments
    let mut doc = toml_content.parse::<toml_edit::DocumentMut>().map_err(|e| format!("Failed to parse Catalyst.toml: {}", e))?;

    // Check if the spark already exists
    let mut spark_already_exists = false;
    if let Some(sparks_table) = doc.get("sparks").and_then(|s| s.as_table()) {
        if sparks_table.contains_key(spark_name) {
            let existing_url = sparks_table.get(spark_name).and_then(|v| v.as_str()).unwrap_or("");
            if existing_url == repo_url {
                logger::info(&format!("Spark {} is already in Catalyst.toml with the same URL", spark_name))?;
                spark_already_exists = true;
            } else {
                logger::warning(&format!("Spark {} is already in Catalyst.toml but with a different URL. Updating...", spark_name))?;
                // Will update the URL below
            }
        }
    }

    // If spark already exists with the same URL, we're done
    if spark_already_exists {
        return Ok(false); // No changes made
    }

    // Make sure the sparks table exists
    if !doc.contains_key("sparks") {
        // First, simply add the sparks table to the document
        // This will add it at the end, keeping the existing order
        doc.insert("sparks", toml_edit::Item::Table(toml_edit::Table::new()));

        // If there's no existing sparks section and we have a settings section,
        // the user might prefer to have sparks section right after settings (default TOML structure)
        if doc.contains_key("settings") {
            // Log what we're doing since this is a structural change
            logger::info("Adding [sparks] section to Catalyst.toml")?;
        }
    }

    // Now add or update the spark in the sparks table
    let sparks_table = doc["sparks"].as_table_mut().unwrap();
    sparks_table[spark_name] = toml_edit::value(repo_url);

    // Write the updated TOML back to the file
    let updated_content = doc.to_string();
    fs::write(config_path, updated_content).map_err(|e| format!("Failed to write updated Catalyst.toml: {}", e))?;

    logger::info(&format!("Updated Catalyst.toml with spark: {} -> {}", spark_name, repo_url))?;
    Ok(true)
}

// Main function to add a spark plugin
pub fn add_spark(repo_url: &str, config: &Config) -> Result<(), String> {
    let mut progress = logger::create_progress(None);
    progress.set_message(&format!("Adding spark plugin from: {}", repo_url));

    // Extract the repo name from the URL to use as the directory name
    let repo_name = extract_repo_name(repo_url)?;
    progress.set_message(&format!("Using repository name: {}", repo_name));

    // Update Catalyst.toml with the spark information
    let _ = update_sparks_toml(&repo_name, repo_url)?;

    // Step 1: Create the sparks directory if it doesn't exist
    let services_dir = Path::new("src").join("services");
    let sparks_dir = services_dir.join("sparks");

    if !services_dir.exists() {
        return Err(format!("Services directory not found. Make sure you're in a Catalyst project."));
    }

    if !sparks_dir.exists() {
        fs::create_dir_all(&sparks_dir).map_err(|e| format!("Failed to create sparks directory: {}", e))?;

        // If the module file doesn't exist, create it with a basic comment
        let mod_rs_path = sparks_dir.join("mod.rs");
        if !mod_rs_path.exists() {
            fs::write(&mod_rs_path, "//here you include the modules you want to expose to the outside world\n").map_err(|e| format!("Failed to create mod.rs file: {}", e))?;
        }
    }

    // Step 2: Create a temporary directory for cloning
    let temp_dir = format!("_temp_spark_{}", repo_name);
    let temp_path = Path::new(&temp_dir);

    // Clean up any existing temporary directory
    if temp_path.exists() {
        fs::remove_dir_all(temp_path).map_err(|e| format!("Failed to clean up temporary directory: {}", e))?;
    }

    // Step 3: Clone the repository
    progress.set_message(&format!("Cloning repository: {}", repo_url));
    let clone_result = Command::new("git")
        .args(&["clone", "--depth=1", repo_url, &temp_dir])
        .output()
        .map_err(|e| format!("Failed to execute git clone: {}", e))?;

    if !clone_result.status.success() {
        return Err(format!("Git clone failed: {}", String::from_utf8_lossy(&clone_result.stderr)));
    }

    // Step 4: Validate the manifest
    progress.set_message("Validating spark manifest...");
    let manifest_path = temp_path.join("manifest.toml");
    if !manifest_path.exists() {
        // Clean up before returning
        let _ = fs::remove_dir_all(temp_path);
        return Err(format!("Spark manifest not found in repository. Expected manifest.toml file."));
    }
    let validation_result = validate_manifest(&manifest_path)?;
    progress.set_message(&format!("Manifest validated: {}", validation_result.name));

    // Step 5: Copy the spark to the final destination
    let target_dir = sparks_dir.join(&repo_name);

    // If the target directory already exists, remove it
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).map_err(|e| format!("Failed to remove existing spark directory: {}", e))?;
    }

    // Copy the repository to the sparks directory
    copy_dir_all(temp_path, &target_dir).map_err(|e| format!("Failed to copy spark to target directory: {}", e))?;

    // Clean up: Remove the temporary directory
    fs::remove_dir_all(temp_path).map_err(|e| format!("Failed to clean up temporary directory: {}", e))?;

    // Step 6: Update the mod.rs file to include the new spark
    update_sparks_mod_rs(&sparks_dir, &repo_name)?;

    // Step 6b: Update the registry.rs file if it exists
    update_spark_registry(&config.project_dir, &repo_name)?;

    // Step 7: Update the project's Cargo.toml with any required dependencies
    if !validation_result.dependencies.is_empty() {
        progress.set_message("Updating Cargo.toml with dependencies...");

        // Log dependency information for debugging
        logger::info(&format!("Found {} dependencies in spark manifest", validation_result.dependencies.len()))?;
        for dep in &validation_result.dependencies {
            let features_str = if dep.features.is_empty() { String::new() } else { format!(" with features: {}", dep.features.join(", ")) };

            let version_str = if let Some(version) = &dep.version { format!(" v{}", version) } else { String::new() };

            logger::info(&format!("  - {}{}{}", dep.crate_name, version_str, features_str))?;
        }

        update_cargo_toml(&validation_result.dependencies)?;
    }

    // Step 8: Check for required environment variables and update .env if needed
    let env_updated = if !validation_result.required_env.is_empty() {
        // Add env variables - finish current progress to disable spinner during editor
        progress.success(&format!("Setting up environment variables for: {}", validation_result.name));

        // This will run without a spinner
        let env_result = update_env_variables(&repo_name, &validation_result.required_env)?;

        // Create a new progress for continuing after the editor
        progress = logger::create_progress(None);
        progress.set_message(&format!("Continuing with spark plugin setup: {}", validation_result.name));

        env_result
    } else {
        false
    };

    // Give a moment for the new env vars to be available
    if env_updated {
        // Load the updated environment variables
        dotenv::dotenv().ok();
        // Brief pause to allow environment to update
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Step 9: Run migrations if any are specified in the manifest
    // We do this after env variables are set so migrations can use them
    if !validation_result.migrations.is_empty() {
        progress.set_message(&format!("Running migrations for spark: {}", validation_result.name));

        for migration in &validation_result.migrations {
            // Construct the full path to the migration directory
            let migration_path = target_dir.join(&migration.path);

            if !migration_path.exists() {
                progress.warning(&format!("Migration path not found: {}", migration_path.display()))?;
                continue;
            }

            // Check if migration directory structure is correct (up.sql/down.sql directly or in named subdirectories)
            let mut direct_migration = false;
            let up_sql = migration_path.join("up.sql");
            if up_sql.exists() {
                direct_migration = true;
                logger::info(&format!("Found direct up.sql migration file: {}", up_sql.display()))?;
            }

            // If not direct, we need to use the parent directory which should contain versioned migration dirs
            let actual_migration_path = if direct_migration {
                // Use the migration path directly since it contains up.sql/down.sql
                migration_path.clone()
            } else {
                // The path should be the parent directory that contains versioned migration dirs
                logger::info(&format!("Using directory for versioned migrations: {}", migration_path.display()))?;
                migration_path.clone()
            };

            progress.set_message(&format!("Running migration: {}", migration.name));

            // Log the path and structure for debugging
            logger::info(&format!("Migration structure: {}", if direct_migration { "direct up.sql/down.sql files" } else { "versioned directories" }))?;

            match run_spark_migration(&actual_migration_path) {
                Ok(_) => {
                    progress.success(&format!("Migration '{}' completed successfully", migration.name));
                }
                Err(e) => {
                    progress.warning(&format!("Migration '{}' failed: {}", migration.name, e))?;
                }
            }
        }
    }

    // Success message
    progress.success(&format!("Successfully added spark plugin: {}", validation_result.name));

    // Report spark information
    logger::info(&format!("Spark Name: {}", validation_result.name))?;
    logger::info(&format!("Version: {}", validation_result.version))?;
    logger::info(&format!("Description: {}", validation_result.description))?;
    logger::info(&format!("Author: {}", validation_result.author))?;

    if !validation_result.dependencies.is_empty() {
        logger::info("Required dependencies:")?;
        for dep in &validation_result.dependencies {
            logger::info(&format!(
                "  - {} {}",
                dep.crate_name,
                if !dep.features.is_empty() { format!("with features: {}", dep.features.join(", ")) } else { String::new() }
            ))?;
        }
    }

    if !validation_result.required_env.is_empty() {
        logger::info(&format!("Required environment variables (added with {}_prefix):", repo_name.to_uppercase()))?;
        for env in &validation_result.required_env {
            // Remove any comments
            let clean_var = env.split('#').next().unwrap_or(env).trim();
            logger::info(&format!("  - {}_{}", repo_name.to_uppercase(), clean_var.to_uppercase()))?;
        }
    }

    Ok(())
}

// Helper struct for migration information
#[derive(Debug)]
struct MigrationInfo {
    name: String,
    path: String,
}

// Helper struct to hold manifest validation results
#[derive(Debug)]
struct ManifestInfo {
    name: String,
    version: String,
    description: String,
    author: String,
    #[allow(dead_code)]
    license: String,
    required_env: Vec<String>,
    dependencies: Vec<Dependency>,
    migrations: Vec<MigrationInfo>,
}

#[derive(Debug)]
struct Dependency {
    crate_name: String,
    version: Option<String>,
    features: Vec<String>,
}

// Function to validate the manifest file
fn validate_manifest(manifest_path: &Path) -> Result<ManifestInfo, String> {
    // Read the manifest file
    let mut file = fs::File::open(manifest_path).map_err(|e| format!("Failed to open manifest file: {}", e))?;

    let mut content = String::new();
    file.read_to_string(&mut content).map_err(|e| format!("Failed to read manifest file: {}", e))?;

    // Parse the TOML
    let parsed = content.parse::<toml::Value>().map_err(|e| format!("Failed to parse manifest TOML: {}", e))?;

    // Validate [spark] section (mandatory)
    let spark = parsed
        .get("spark")
        .ok_or_else(|| format!("Missing [spark] section in manifest"))?
        .as_table()
        .ok_or_else(|| format!("[spark] section must be a table"))?;

    // Extract mandatory fields from [spark]
    let name = spark
        .get("name")
        .ok_or_else(|| format!("Missing name in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("name must be a string"))?
        .to_string();

    let version = spark
        .get("version")
        .ok_or_else(|| format!("Missing version in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("version must be a string"))?
        .to_string();

    let description = spark
        .get("description")
        .ok_or_else(|| format!("Missing description in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("description must be a string"))?
        .to_string();

    let author = spark
        .get("author")
        .ok_or_else(|| format!("Missing author in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("author must be a string"))?
        .to_string();

    let license = spark
        .get("license")
        .ok_or_else(|| format!("Missing license in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("license must be a string"))?
        .to_string();

    // Extract optional fields

    // Parse dependencies - now supporting two formats:
    // 1. The original format with an array of features
    // 2. The new direct format mapping crate names to version/features
    let mut dependencies = Vec::new();
    if let Some(deps_section) = parsed.get("dependencies") {
        if let Some(deps_table) = deps_section.as_table() {
            // Format 1 (original)
            if let Some(features) = deps_table.get("features") {
                if let Some(features_array) = features.as_array() {
                    for feature in features_array {
                        if let Some(feature_table) = feature.as_table() {
                            let crate_name = feature_table.get("crate_name").and_then(|v| v.as_str()).ok_or_else(|| format!("Missing crate_name in dependency"))?.to_string();

                            let mut feature_list = Vec::new();
                            if let Some(features_value) = feature_table.get("features") {
                                if let Some(features) = features_value.as_array() {
                                    for f in features {
                                        if let Some(f_str) = f.as_str() {
                                            feature_list.push(f_str.to_string());
                                        }
                                    }
                                }
                            }

                            dependencies.push(Dependency {
                                crate_name,
                                version: None,
                                features: feature_list,
                            });
                        }
                    }
                }
            }

            // Format 2 (new direct format)
            // Process any directly defined crates in the dependencies table
            for (crate_name, crate_info) in deps_table {
                // Skip the "features" entry we processed above
                if crate_name == "features" {
                    continue;
                }

                let mut version = None;
                let mut features = Vec::new();

                // Handle simple string version
                if let Some(ver_str) = crate_info.as_str() {
                    version = Some(ver_str.to_string());
                }
                // Handle table with version and features
                else if let Some(crate_table) = crate_info.as_table() {
                    if let Some(ver) = crate_table.get("version").and_then(|v| v.as_str()) {
                        version = Some(ver.to_string());
                    }

                    // Handle features array
                    if let Some(feats) = crate_table.get("features") {
                        // Try as array first
                        if let Some(feats_array) = feats.as_array() {
                            for feat in feats_array {
                                if let Some(feat_str) = feat.as_str() {
                                    features.push(feat_str.to_string());
                                }
                            }
                        }
                        // Try as string (sometimes features can be specified as a single string)
                        else if let Some(feat_str) = feats.as_str() {
                            features.push(feat_str.to_string());
                        }
                    }
                }

                dependencies.push(Dependency {
                    crate_name: crate_name.to_string(),
                    version,
                    features,
                });
            }
        }
    }

    // Parse required environment variables
    let mut required_env = Vec::new();
    if let Some(config_section) = parsed.get("config") {
        if let Some(config_table) = config_section.as_table() {
            if let Some(env_vars) = config_table.get("required_env") {
                if let Some(env_array) = env_vars.as_array() {
                    for env in env_array {
                        if let Some(env_str) = env.as_str() {
                            required_env.push(env_str.to_string());
                        }
                    }
                }
            }
        }
    }

    // Parse migrations
    let mut migrations = Vec::new();
    if let Some(migrations_section) = parsed.get("migrations") {
        if let Some(migrations_array) = migrations_section.as_array() {
            for migration in migrations_array {
                if let Some(migration_table) = migration.as_table() {
                    let migration_name = migration_table.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed").to_string();

                    let migration_path = migration_table.get("path").and_then(|v| v.as_str()).unwrap_or("migrations").to_string();

                    migrations.push(MigrationInfo {
                        name: migration_name,
                        path: migration_path,
                    });
                }
            }
        }
    }

    // All mandatory fields are present, return successful validation
    Ok(ManifestInfo {
        name,
        version,
        description,
        author,
        license,
        required_env,
        dependencies,
        migrations,
    })
}

// Helper function to extract the repository name from the URL
fn extract_repo_name(repo_url: &str) -> Result<String, String> {
    let url = url::Url::parse(repo_url).map_err(|e| format!("Invalid URL: {}", e))?;

    let path = url.path().trim_end_matches('/');
    let name = path.split('/').last().ok_or_else(|| format!("Could not extract repository name from URL"))?;

    // Remove .git suffix if present
    let name = name.trim_end_matches(".git");

    if name.is_empty() {
        return Err(format!("Empty repository name extracted from URL"));
    }

    Ok(name.to_string())
}

// Helper function to copy a directory recursively
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;

        // Skip .git directory
        if ty.is_dir() && entry.file_name() == ".git" {
            continue;
        }

        let dst_path = dst.as_ref().join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(entry.path(), dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }

    Ok(())
}

// Helper function to update the project's Cargo.toml with the spark dependencies
fn update_cargo_toml(dependencies: &[Dependency]) -> Result<(), String> {
    let cargo_path = Path::new("Cargo.toml");

    if !cargo_path.exists() {
        return Err(format!("Cargo.toml not found in the current directory. Make sure you're in a valid Catalyst project."));
    }

    // Read the current Cargo.toml
    let cargo_content = fs::read_to_string(cargo_path).map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;

    // Parse Cargo.toml
    let mut cargo_doc = cargo_content.parse::<toml_edit::DocumentMut>().map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;

    // Get or create the dependencies table
    let deps_table = if cargo_doc.contains_key("dependencies") {
        // Table exists, get mutable reference
        cargo_doc["dependencies"].as_table_mut().ok_or_else(|| format!("Failed to access dependencies table in Cargo.toml"))?
    } else {
        // Table doesn't exist, create it
        cargo_doc["dependencies"] = toml_edit::Item::Table(toml_edit::Table::new());
        cargo_doc["dependencies"].as_table_mut().ok_or_else(|| format!("Failed to create dependencies table in Cargo.toml"))?
    };

    // Track changes made to Cargo.toml
    let mut changes_made = false;
    let mut changes_summary = Vec::new();

    // Process each dependency
    for dep in dependencies {
        let dep_name = &dep.crate_name;

        if deps_table.contains_key(dep_name) {
            // Dependency already exists, check if we need to update features
            if !dep.features.is_empty() {
                let existing_dep = &mut deps_table[dep_name];

                // Log the type of the dependency for debugging
                logger::info(&format!(
                    "Dependency {} found with type: {}",
                    dep_name,
                    if existing_dep.is_str() {
                        "string"
                    } else if existing_dep.is_table() {
                        "table"
                    } else if existing_dep.is_inline_table() {
                        "inline_table"
                    } else {
                        "other"
                    }
                ))?;

                // If it's a simple string, convert it to an inline table
                if existing_dep.is_str() {
                    logger::info(&format!("Converting simple string dependency to table with features: {}", dep_name))?;
                    let version = existing_dep.as_str().unwrap_or_default().to_string();
                    // Create new inline table with version and features
                    let mut inline_table = toml_edit::InlineTable::new();
                    inline_table.insert("version", version.into());

                    let mut features_array = toml_edit::Array::new();
                    for feature in &dep.features {
                        features_array.push(toml_edit::Value::from(feature.clone()));
                        logger::info(&format!("  Adding feature: {}", feature))?;
                    }

                    inline_table.insert("features", toml_edit::Value::from(features_array));

                    // Replace the simple version string with the inline table
                    *existing_dep = toml_edit::Item::Value(toml_edit::Value::InlineTable(inline_table));
                    changes_made = true;
                    changes_summary.push(format!("Updated {} to include features: {}", dep_name, dep.features.join(", ")));
                }
                // If it's an inline table (commonly used in Cargo.toml)
                else if existing_dep.is_inline_table() {
                    logger::info(&format!("Handling inline table dependency: {}", dep_name))?;

                    if let Some(inline_table) = existing_dep.as_inline_table_mut() {
                        let mut existing_features = Vec::new();
                        let mut missing_features = Vec::new();

                        // Get existing features
                        if let Some(features_value) = inline_table.get("features") {
                            if let Some(features_array) = features_value.as_array() {
                                for feat in features_array.iter() {
                                    if let Some(feat_str) = feat.as_str() {
                                        existing_features.push(feat_str.to_string());
                                        logger::info(&format!("  Found existing feature: {}", feat_str))?;
                                    }
                                }
                            }
                        }

                        // Find missing features
                        for feature in &dep.features {
                            if !existing_features.contains(feature) {
                                missing_features.push(feature.clone());
                                logger::info(&format!("  Adding missing feature: {}", feature))?;
                            }
                        }

                        // Update features if needed
                        if !missing_features.is_empty() {
                            let mut all_features = existing_features.clone();
                            all_features.extend(missing_features.iter().cloned());

                            // Create new array with all features
                            let mut features_array = toml_edit::Array::new();
                            for feature in all_features {
                                features_array.push(toml_edit::Value::from(feature));
                            }

                            // Update the features field
                            inline_table.insert("features", toml_edit::Value::from(features_array));
                            changes_made = true;
                            changes_summary.push(format!("Added features to {}: {}", dep_name, missing_features.join(", ")));
                        }
                    }
                }
                // If it's already a table, add or update features
                else if let Some(dep_table) = existing_dep.as_table_mut() {
                    let mut existing_features = Vec::new();
                    let mut missing_features = Vec::new();

                    // Log what we're working with
                    logger::info(&format!("Examining existing dependency: {}", dep_name))?;

                    // Get existing features - handle both array and string formats
                    if let Some(features_item) = dep_table.get("features") {
                        // Try as array first
                        if let Some(feat_array) = features_item.as_array() {
                            for feat in feat_array.iter() {
                                if let Some(feat_str) = feat.as_str() {
                                    existing_features.push(feat_str.to_string());
                                    logger::info(&format!("  Found existing feature: {}", feat_str))?;
                                }
                            }
                        }
                        // Try as string
                        else if let Some(feat_str) = features_item.as_str() {
                            existing_features.push(feat_str.to_string());
                            logger::info(&format!("  Found existing feature (string): {}", feat_str))?;
                        }
                    }

                    // Find missing features
                    logger::info(&format!("Looking for features to add to {}: {}", dep_name, dep.features.join(", ")))?;
                    for feature in &dep.features {
                        if !existing_features.contains(feature) {
                            missing_features.push(feature.clone());
                            logger::info(&format!("  Adding missing feature: {}", feature))?;
                        } else {
                            logger::info(&format!("  Feature already exists: {}", feature))?;
                        }
                    }

                    // Update features if needed
                    if !missing_features.is_empty() {
                        let mut new_features = existing_features.clone();
                        new_features.extend(missing_features.iter().cloned());

                        // Create new features array
                        let mut features_array = toml_edit::Array::new();
                        for feature in new_features {
                            features_array.push(toml_edit::Value::from(feature));
                        }

                        // Update the features
                        dep_table.insert("features", toml_edit::Item::Value(toml_edit::Value::from(features_array)));
                        changes_made = true;
                        changes_summary.push(format!("Added features to {}: {}", dep_name, missing_features.join(", ")));
                    }
                }
            }
        } else {
            // Dependency doesn't exist, add it
            if dep.features.is_empty() {
                // Simple dependency without features
                if let Some(version) = &dep.version {
                    deps_table.insert(dep_name, toml_edit::Item::Value(toml_edit::Value::from(version.clone())));
                } else {
                    // If no version provided, use a default latest version
                    deps_table.insert(dep_name, toml_edit::Item::Value(toml_edit::Value::from("*")));
                }
                changes_made = true;
                changes_summary.push(format!("Added dependency: {}", dep_name));
            } else {
                // Dependency with features
                let mut inline_table = toml_edit::InlineTable::new();

                // Add version if available, otherwise use wildcard
                if let Some(version) = &dep.version {
                    inline_table.insert("version", version.clone().into());
                } else {
                    inline_table.insert("version", "*".into());
                }

                // Add features
                let mut features_array = toml_edit::Array::new();
                for feature in &dep.features {
                    features_array.push(toml_edit::Value::from(feature.clone()));
                }
                inline_table.insert("features", toml_edit::Value::from(features_array));

                // Add the complete dependency
                deps_table.insert(dep_name, toml_edit::Item::Value(toml_edit::Value::InlineTable(inline_table)));
                changes_made = true;
                changes_summary.push(format!("Added dependency: {} with features: {}", dep_name, dep.features.join(", ")));
            }
        }
    }

    if changes_made {
        // Write the updated Cargo.toml
        let cargo_string = cargo_doc.to_string();

        // Log the first few lines for debugging
        logger::info(&format!("Writing Cargo.toml to: {}", cargo_path.display()))?;

        match fs::write(cargo_path, cargo_string) {
            Ok(_) => {
                // Log the changes
                logger::info("Updated Cargo.toml with the following changes:")?;
                for change in changes_summary {
                    logger::info(&format!("  - {}", change))?;
                }
                logger::info("You may need to run 'cargo build' to update your dependencies")?;
            }
            Err(e) => {
                return Err(format!("Failed to write updated Cargo.toml: {}. Make sure you have write permissions to the file.", e));
            }
        }
    } else {
        logger::info("No changes needed to Cargo.toml - dependencies already satisfied")?;
    }

    Ok(())
}

// Helper function to check for and update required environment variables
fn update_env_variables(spark_name: &str, required_env: &[String]) -> Result<bool, String> {
    // Find the .env file
    let env_path = Path::new(".env");
    if !env_path.exists() {
        return Err(format!(".env file not found. Make sure you're in a valid Catalyst project."));
    }

    // Read the current .env file
    let env_content = fs::read_to_string(env_path).map_err(|e| format!("Failed to read .env file: {}", e))?;

    // Parse existing variables and check if they have placeholder values
    let mut existing_vars = std::collections::HashSet::new();
    let mut placeholder_vars = std::collections::HashSet::new();

    for line in env_content.lines() {
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }

        // Extract variable name and value
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            let var_name = parts[0].trim().to_string();
            let var_value = parts[1].trim().to_string();

            existing_vars.insert(var_name.clone());

            // Check if this is a placeholder value
            if var_value.contains("REPLACE_THIS_WITH_YOUR_VALUE") {
                placeholder_vars.insert(var_name);
            }
        }
    }

    // Create spark-specific variable name format (SPARKNAME_VARNAME)
    let spark_prefix = format!("{}_", spark_name.to_uppercase());

    // Check for missing or placeholder variables
    let mut vars_to_update = Vec::new();
    for env_var in required_env {
        // Parse out any comments
        let clean_var = env_var.split('#').next().unwrap_or(env_var).trim();
        let comment = if let Some(comment_part) = env_var.split('#').nth(1) {
            format!("# {}", comment_part.trim())
        } else {
            String::new()
        };

        // Create the spark-specific variable name
        let spark_var_name = format!("{}{}", spark_prefix, clean_var.to_uppercase());

        // Check if variable is missing OR has a placeholder value
        if !existing_vars.contains(&spark_var_name) || placeholder_vars.contains(&spark_var_name) {
            vars_to_update.push((spark_var_name, comment));
        }
    }

    // If there are variables to update, add them to .env and open editor
    if !vars_to_update.is_empty() {
        // If we have placeholders, first remove them and their comment section from the file
        if !placeholder_vars.is_empty() {
            let spark_vars: Vec<_> = placeholder_vars.iter().filter(|v| v.starts_with(&spark_prefix)).collect();

            if !spark_vars.is_empty() {
                // Read all lines
                let mut all_lines: Vec<String> = Vec::new();
                let mut skip_next_comment = false;
                let comment_str = format!("# Environment variables for {} spark", spark_name);

                for line in env_content.lines() {
                    // Skip any comment lines for this spark
                    if line.trim() == comment_str {
                        skip_next_comment = true;
                        continue;
                    }

                    // Skip blank lines right after spark comments
                    if skip_next_comment && line.trim().is_empty() {
                        skip_next_comment = false;
                        continue;
                    }

                    // Keep lines that don't start with our placeholder vars
                    let parts: Vec<&str> = line.splitn(2, '=').collect();
                    if parts.len() >= 1 {
                        let var_name = parts[0].trim();
                        if spark_vars.iter().any(|v| v == &var_name) {
                            continue; // Skip this var
                        }
                    }

                    all_lines.push(line.to_string());
                }

                // Write back without the placeholder lines and comments
                fs::write(env_path, all_lines.join("\n")).map_err(|e| format!("Failed to update .env file: {}", e))?;
            }
        }

        // Now append the new variables
        let mut env_file = fs::OpenOptions::new().append(true).open(env_path).map_err(|e| format!("Failed to open .env file for writing: {}", e))?;

        // Add a comment indicating these are for the spark
        writeln!(env_file, "\n# Environment variables for {} spark", spark_name).map_err(|e| format!("Failed to write to .env file: {}", e))?;

        // Add each variable with a placeholder value
        for (name, comment) in &vars_to_update {
            // Write as NAME="YOUR_VALUE_HERE" format with comment if available
            let line = if comment.is_empty() {
                format!("{}=\"REPLACE_THIS_WITH_YOUR_VALUE\"", name)
            } else {
                format!("{}=\"REPLACE_THIS_WITH_YOUR_VALUE\" {}", name, comment)
            };

            writeln!(env_file, "{}", line).map_err(|e| format!("Failed to write to .env file: {}", e))?;
        }

        // Close the file
        drop(env_file);

        // Inform the user what we've done
        logger::info(&format!("⚠️  Required environment variables for spark '{}' have been added to your .env file", spark_name))?;
        logger::info("   Please replace the placeholder values with your actual values.")?;

        // Clear the screen and notify about editor
        println!("\n\nOpening .env file in your editor so you can set the values...");

        // Try to open with EDITOR env var
        let editor_result = if let Ok(editor) = std::env::var("EDITOR") {
            println!("Using editor: {}", editor);

            // Sleep to make sure the editor has our full attention
            std::thread::sleep(std::time::Duration::from_secs(1));

            match Command::new(&editor).arg(env_path).status() {
                Ok(status) if status.success() => {
                    logger::info("File has been successfully edited")?;
                    true
                }
                _ => {
                    logger::warning("Could not open editor from EDITOR environment variable")?;
                    false
                }
            }
        } else {
            false
        };

        // If EDITOR didn't work, try common editors
        if !editor_result {
            open_with_common_editors(env_path)?;
        }

        // Check specifically for spark env vars with placeholder values
        let updated_content = fs::read_to_string(env_path).map_err(|e| format!("Failed to read .env file after editing: {}", e))?;

        // Look for our specific spark env variables that still have placeholder values
        let spark_prefix = format!("{}_", spark_name.to_uppercase());
        let mut placeholders_found = false;

        // Check every line for the pattern SPARKNAME_VARNAME="REPLACE_THIS_WITH_YOUR_VALUE"
        for line in updated_content.lines() {
            if line.trim().starts_with(&spark_prefix) && line.contains("REPLACE_THIS_WITH_YOUR_VALUE") {
                placeholders_found = true;
                break;
            }
        }

        if placeholders_found {
            logger::warning(&format!("Some environment variables for {} still have placeholder values!", spark_name))?;
            logger::warning(&format!("Please manually edit the .env file at: {}", env_path.display()))?;
            logger::warning("Replace the REPLACE_THIS_WITH_YOUR_VALUE placeholders with actual values")?;
        } else {
            logger::success("All environment variables have been set")?;
        }
    } else {
        logger::success("All required environment variables are already set")?;
    }

    Ok(true)
}

// Helper function to try opening a file with common editors
fn open_with_common_editors(file_path: &Path) -> Result<(), String> {
    // Clear terminal and display message
    println!("\n\nOpening editor for .env file. Please edit the environment variables...\n");
    println!("When you're done, save and close the editor to continue.\n");

    // Give the user time to read the message
    std::thread::sleep(std::time::Duration::from_secs(1));

    let mut editor_found = false;

    #[cfg(target_os = "windows")]
    {
        // On Windows, try notepad
        if Command::new("which").arg("notepad").output().map(|o| o.status.success()).unwrap_or(false) {
            logger::info("Opening .env file with notepad...")?;

            // Open the editor and wait for it to complete
            if let Ok(status) = Command::new("notepad").arg(file_path).status() {
                if status.success() {
                    editor_found = true;
                    logger::info("File has been edited with notepad")?;
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Unix-like systems, try various common editors
        let editors = ["nano", "vim", "vi", "gedit", "code", "emacs", "subl", "pico"];

        for editor in editors {
            // Try to check if the editor is installed
            if Command::new("which").arg(editor).output().map(|o| o.status.success()).unwrap_or(false) {
                // Editor exists, try to use it
                logger::info(&format!("Opening .env file with {}...", editor))?;

                // Small pause before starting the editor
                std::thread::sleep(std::time::Duration::from_millis(200));

                if let Ok(status) = Command::new(editor).arg(file_path).status() {
                    if status.success() {
                        editor_found = true;
                        logger::info(&format!("File has been edited with {}", editor))?;
                        break;
                    }
                }
            }
        }
    }

    // If we got here and didn't find an editor, log the error
    if !editor_found {
        logger::warning(&format!("Could not find a suitable editor to open {}", file_path.display()))?;
        logger::warning("Please edit the file manually to set environment variables")?;
    }

    // Give the user a moment to see the messages
    std::thread::sleep(std::time::Duration::from_secs(1));

    Ok(())
}

// Function to check for and install sparks in Catalyst.toml
pub fn sync_sparks_from_config(config: &Config) -> Result<(), String> {
    use std::path::Path;

    // First, check if Catalyst.toml exists and load it directly
    let catalyst_toml_path = Path::new("Catalyst.toml");
    if !catalyst_toml_path.exists() {
        return Err("Catalyst.toml not found. Make sure you're in a Catalyst project directory.".to_string());
    }

    // Read and parse Catalyst.toml directly
    let toml_content = fs::read_to_string(catalyst_toml_path)
        .map_err(|e| format!("Failed to read Catalyst.toml: {}", e))?;
    
    let catalyst_config: toml::Value = toml::from_str(&toml_content)
        .map_err(|e| format!("Failed to parse Catalyst.toml: {}", e))?;

    // Get the sparks section
    let sparks_section = match catalyst_config.get("sparks") {
        Some(sparks) => sparks.as_table().ok_or("Sparks section must be a table")?,
        None => {
            logger::info("No sparks section found in Catalyst.toml")?;
            return Ok(());
        }
    };

    logger::info(&format!("Found {} configured spark(s) in Catalyst.toml", sparks_section.len()))?;

    // Check which sparks are already installed
    let sparks_dir = Path::new("src/services/sparks");
    let mut missing_sparks = Vec::new();
    let mut installed_sparks = Vec::new();

    // Only check configured sparks if there are any
    if !sparks_section.is_empty() {
        for (spark_name, spark_url) in sparks_section {
            let spark_dir = sparks_dir.join(spark_name);
            let manifest_path = spark_dir.join("manifest.toml");
            
            if spark_dir.exists() && manifest_path.exists() {
                // Verify the manifest is valid
                match validate_manifest(&manifest_path) {
                    Ok(_) => {
                        // Check if spark is properly registered in registry.rs and mod.rs
                        let registry_missing = !check_spark_in_registry(spark_name, &config.project_dir)?;
                        let mod_missing = !check_spark_in_mod_rs(spark_name, sparks_dir)?;
                        
                        if registry_missing || mod_missing {
                            logger::warning(&format!("Spark '{}' directory exists but is incomplete:", spark_name))?;
                            if registry_missing {
                                logger::warning(&format!("  - Missing from registry.rs"))?;
                            }
                            if mod_missing {
                                logger::warning(&format!("  - Missing from mod.rs"))?;
                            }
                            
                            // Repair the spark registration instead of re-downloading
                            logger::info(&format!("Repairing spark registration for: {}", spark_name))?;
                            if let Err(e) = repair_spark_registration(spark_name, sparks_dir, &config.project_dir) {
                                logger::warning(&format!("Failed to repair spark '{}': {}", spark_name, e))?;
                                // If repair fails, add to missing sparks for full reinstall
                                if let Some(url) = spark_url.as_str() {
                                    missing_sparks.push((spark_name.clone(), url.to_string()));
                                }
                            } else {
                                installed_sparks.push(spark_name.clone());
                                logger::success(&format!("✓ Repaired spark '{}'", spark_name))?;
                            }
                        } else {
                            installed_sparks.push(spark_name.clone());
                            logger::info(&format!("✓ Spark '{}' is fully installed and valid", spark_name))?;
                        }
                    }
                    Err(e) => {
                        logger::warning(&format!("Spark '{}' is installed but has invalid manifest: {}", spark_name, e))?;
                        if let Some(url) = spark_url.as_str() {
                            missing_sparks.push((spark_name.clone(), url.to_string()));
                        }
                    }
                }
            } else {
                if let Some(url) = spark_url.as_str() {
                    missing_sparks.push((spark_name.clone(), url.to_string()));
                } else {
                    logger::warning(&format!("Invalid URL for spark '{}' in Catalyst.toml", spark_name))?;
                }
            }
        }
    }

    // Check for installed sparks that are not configured and should be removed
    let mut unconfigured_sparks = Vec::new();
    
    if sparks_dir.exists() {
        if let Ok(entries) = fs::read_dir(sparks_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    
                    if !path.is_dir() || ["registry", "makeuse"].contains(&path.file_name().unwrap_or_default().to_string_lossy().as_ref()) {
                        continue;
                    }
                    
                    let spark_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    
                    // If spark is installed but not configured, mark for removal
                    if !sparks_section.contains_key(&spark_name) {
                        unconfigured_sparks.push(spark_name);
                    }
                }
            }
        }
    }
    
    // Check for orphaned registry entries (sparks in registry.rs but not configured)
    let mut orphaned_registry_entries = Vec::new();
    if let Ok(registry_sparks) = get_sparks_from_registry(&config.project_dir) {
        for registry_spark in registry_sparks {
            if !sparks_section.contains_key(&registry_spark) {
                orphaned_registry_entries.push(registry_spark);
            }
        }
    }

    // Remove unconfigured sparks (from directories)
    if !unconfigured_sparks.is_empty() {
        logger::info(&format!("Removing {} unconfigured spark(s): {:?}", unconfigured_sparks.len(), unconfigured_sparks))?;
        
        for spark_name in unconfigured_sparks {
            match remove_spark(&spark_name, config) {
                Ok(_) => {
                    logger::success(&format!("Removed unconfigured spark: {}", spark_name))?;
                }
                Err(e) => {
                    logger::warning(&format!("Failed to remove spark {}: {}", spark_name, e))?;
                }
            }
        }
    }

    // Clean up orphaned registry entries
    if !orphaned_registry_entries.is_empty() {
        logger::info(&format!("Cleaning up {} orphaned registry entries: {:?}", orphaned_registry_entries.len(), orphaned_registry_entries))?;
        
        for spark_name in orphaned_registry_entries {
            if let Err(e) = remove_from_spark_registry(&config.project_dir, &spark_name) {
                logger::warning(&format!("Failed to remove '{}' from registry: {}", spark_name, e))?;
            } else {
                logger::success(&format!("Removed orphaned registry entry: {}", spark_name))?;
            }
        }
    }

    if missing_sparks.is_empty() {
        logger::success("All sparks are synchronized")?;
        return Ok(());
    }

    logger::info(&format!("Installing {} missing spark(s)", missing_sparks.len()))?;

    // Setup progress bar for installation
    let total_sparks = missing_sparks.len() as u64;
    let mut progress = logger::create_progress(Some(total_sparks));
    progress.set_message("Syncing missing sparks...");

    let mut current = 0;
    for (spark_name, spark_url) in missing_sparks {
        current += 1;
        progress.set_message(&format!("Installing spark ({}/{}): {}", current, total_sparks, spark_name));

        // Attempt to add the spark
        match add_spark(&spark_url, config) {
            Ok(_) => {
                progress.set_message(&format!("Installed spark ({}/{}): {}", current, total_sparks, spark_name));
                logger::success(&format!("Successfully installed spark: {}", spark_name))?;
            }
            Err(e) => {
                progress.warning(&format!("Failed to install spark {}: {}", spark_name, e))?;
            }
        }
        progress.inc(1);
    }

    progress.success("Spark synchronization complete!");
    Ok(())
}

// Function to remove a spark from the project
pub fn remove_spark(spark_name: &str, config: &Config) -> Result<(), String> {
    let sparks_dir = Path::new("src/services/sparks");
    let spark_dir = sparks_dir.join(spark_name);
    
    if !spark_dir.exists() {
        logger::warning(&format!("Spark '{}' directory not found, checking other locations", spark_name))?;
    }

    // Safety check: don't remove core spark files
    if ["makeuse", "registry", "mod"].contains(&spark_name) {
        return Err(format!("Cannot remove core spark component '{}'", spark_name));
    }

    logger::info(&format!("Safely removing spark: {}", spark_name))?;
    
    // First update the configuration files before removing the directory
    // This way if file updates fail, we still have the spark intact
    
    // Update mod.rs to remove the module declaration (creates backup)
    if let Err(e) = remove_from_sparks_mod_rs(sparks_dir, spark_name) {
        logger::warning(&format!("Failed to update mod.rs: {}", e))?;
    }
    
    // Update registry.rs to remove the match arm (creates backup)
    if let Err(e) = remove_from_spark_registry(&config.project_dir, spark_name) {
        logger::warning(&format!("Failed to update registry.rs: {}", e))?;
    }
    
    // Remove from Catalyst.toml
    if let Err(e) = remove_from_sparks_toml(spark_name) {
        logger::warning(&format!("Failed to update Catalyst.toml: {}", e))?;
    }
    
    // Finally remove the spark directory if it exists
    if spark_dir.exists() {
        logger::info(&format!("Removing spark directory: {}", spark_dir.display()))?;
        fs::remove_dir_all(&spark_dir)
            .map_err(|e| format!("Failed to remove spark directory: {}", e))?;
        logger::info(&format!("Removed spark directory: {}", spark_dir.display()))?;
    }
    
    logger::success(&format!("Successfully removed spark: {}", spark_name))?;
    Ok(())
}

// Helper function to remove spark from mod.rs
fn remove_from_sparks_mod_rs(sparks_dir: &Path, spark_name: &str) -> Result<(), String> {
    let mod_rs_path = sparks_dir.join("mod.rs");
    
    if !mod_rs_path.exists() {
        logger::info("mod.rs doesn't exist, nothing to update")?;
        return Ok(()); // mod.rs doesn't exist, nothing to remove
    }
    
    let content = fs::read_to_string(&mod_rs_path)
        .map_err(|e| format!("Failed to read mod.rs: {}", e))?;
    
    let module_line = format!("pub mod {};", spark_name);
    
    // Check if the module line actually exists
    if !content.contains(&module_line) {
        logger::info(&format!("Module '{}' not found in mod.rs, nothing to remove", spark_name))?;
        return Ok(());
    }
    
    // Create backup before modifying
    let backup_path = mod_rs_path.with_extension("rs.backup");
    fs::write(&backup_path, &content)
        .map_err(|e| format!("Failed to create backup: {}", e))?;
    logger::info(&format!("Created backup at {}", backup_path.display()))?;
    
    let updated_content = content
        .lines()
        .filter(|line| line.trim() != module_line.trim())
        .collect::<Vec<_>>()
        .join("\n");
    
    // Verify the content changed
    if updated_content == content {
        logger::info(&format!("No changes needed for '{}' in mod.rs", spark_name))?;
        // Remove backup since we didn't change anything
        let _ = fs::remove_file(&backup_path);
        return Ok(());
    }
    
    // Basic validation: ensure we still have the essential modules
    if !updated_content.contains("pub mod makeuse") || !updated_content.contains("pub mod registry") {
        // Remove backup since validation failed
        let _ = fs::remove_file(&backup_path);
        return Err("Safety check failed: essential modules (makeuse, registry) would be removed".to_string());
    }
    
    fs::write(&mod_rs_path, updated_content)
        .map_err(|e| format!("Failed to update mod.rs: {}", e))?;
    
    logger::success(&format!("Safely removed '{}' from mod.rs", spark_name))?;
    logger::info(&format!("Backup saved at {}", backup_path.display()))?;
    
    Ok(())
}

// Helper function to remove spark from registry.rs
fn remove_from_spark_registry(project_dir: &Path, spark_name: &str) -> Result<(), String> {
    let registry_path = project_dir.join("src").join("services").join("sparks").join("registry.rs");
    
    if !registry_path.exists() {
        logger::info("registry.rs doesn't exist, nothing to update")?;
        return Ok(()); // registry.rs doesn't exist, nothing to remove
    }
    
    let content = fs::read_to_string(&registry_path)
        .map_err(|e| format!("Failed to read registry.rs: {}", e))?;
    
    // First check if the spark is actually present in the registry
    let pattern = format!("\"{spark_name}\" =>");
    if !content.contains(&pattern) {
        logger::info(&format!("Spark '{}' not found in registry.rs, nothing to remove", spark_name))?;
        return Ok(());
    }
    
    // Create backup before modifying
    let backup_path = registry_path.with_extension("rs.backup");
    fs::write(&backup_path, &content)
        .map_err(|e| format!("Failed to create backup: {}", e))?;
    logger::info(&format!("Created backup at {}", backup_path.display()))?;
    
    // Remove the match arm for this spark more safely
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines = Vec::new();
    let mut in_spark_match_arm = false;
    let mut brace_count = 0;
    let mut found_spark = false;
    
    for line in lines {
        let trimmed = line.trim();
        
        // Check if we're starting the specific spark's match arm (not the wildcard)
        if trimmed.starts_with(&pattern) && !trimmed.starts_with("_ =>") {
            in_spark_match_arm = true;
            found_spark = true;
            brace_count = 0;
            continue; // Skip the match arm line
        }
        
        if in_spark_match_arm {
            // Count braces to track nesting
            for ch in line.chars() {
                match ch {
                    '{' => brace_count += 1,
                    '}' => brace_count -= 1,
                    _ => {}
                }
            }
            
            // If we've closed all braces and the line ends with }, we're done with this match arm
            if brace_count <= 0 && (trimmed.ends_with("},") || trimmed == "}," || trimmed == "}") {
                in_spark_match_arm = false;
                continue; // Skip the closing brace line
            }
            continue; // Skip all lines within the match arm
        }
        
        new_lines.push(line);
    }
    
    if !found_spark {
        logger::warning(&format!("Spark '{}' pattern found but couldn't parse match arm safely", spark_name))?;
        // Remove backup since we didn't change anything
        let _ = fs::remove_file(&backup_path);
        return Ok(());
    }
    
    let updated_content = new_lines.join("\n");
    
    // Verify the content changed and looks valid
    if updated_content == content {
        logger::info(&format!("No changes needed for '{}' in registry.rs", spark_name))?;
        // Remove backup since we didn't change anything
        let _ = fs::remove_file(&backup_path);
        return Ok(());
    }
    
    // Basic validation: ensure we still have the register_by_name function
    if !updated_content.contains("pub fn register_by_name") {
        return Err("Safety check failed: register_by_name function would be removed".to_string());
    }
    
    // Check if match statement is empty and fix it
    let final_content = if updated_content.contains("match name {\n        \n    }") || 
                          updated_content.contains("match name {\n    \n    }") ||
                          updated_content.contains("match name {\n    }") ||
                          updated_content.contains("match name {}") {
        logger::info("Fixing empty match statement in registry.rs")?;
        // Find and replace various forms of empty match statements
        let mut fixed = updated_content.clone();
        
        // Replace different patterns of empty match statements
        let patterns = [
            ("match name {\n        \n    }", "match name {\n        _ => {\n            cata_log!(Warning, format!(\"Cannot register unknown spark '{}'\", name));\n            false\n        }\n    }"),
            ("match name {\n    \n    }", "match name {\n        _ => {\n            cata_log!(Warning, format!(\"Cannot register unknown spark '{}'\", name));\n            false\n        }\n    }"),
            ("match name {\n    }", "match name {\n        _ => {\n            cata_log!(Warning, format!(\"Cannot register unknown spark '{}'\", name));\n            false\n        }\n    }"),
            ("match name {}", "match name {\n        _ => {\n            cata_log!(Warning, format!(\"Cannot register unknown spark '{}'\", name));\n            false\n        }\n    }"),
        ];
        
        for (pattern, replacement) in &patterns {
            if fixed.contains(pattern) {
                fixed = fixed.replace(pattern, replacement);
                break;
            }
        }
        
        fixed
    } else {
        updated_content
    };
    
    // Write the final content
    fs::write(&registry_path, final_content)
        .map_err(|e| format!("Failed to update registry.rs: {}", e))?;
    
    logger::success(&format!("Safely removed '{}' from registry.rs", spark_name))?;
    logger::info(&format!("Backup saved at {}", backup_path.display()))?;
    
    Ok(())
}

// Helper function to remove spark from Catalyst.toml
fn remove_from_sparks_toml(spark_name: &str) -> Result<(), String> {
    let config_path = Path::new("Catalyst.toml");
    if !config_path.exists() {
        return Ok(()); // No Catalyst.toml to update
    }
    
    let toml_content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read Catalyst.toml: {}", e))?;
    
    let mut doc = toml_content.parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Failed to parse Catalyst.toml: {}", e))?;
    
    // Remove the spark from the sparks table if it exists
    if let Some(sparks_table) = doc.get_mut("sparks").and_then(|s| s.as_table_mut()) {
        if sparks_table.contains_key(spark_name) {
            sparks_table.remove(spark_name);
            
            let updated_content = doc.to_string();
            fs::write(config_path, updated_content)
                .map_err(|e| format!("Failed to write updated Catalyst.toml: {}", e))?;
            
            logger::info(&format!("Removed '{}' from Catalyst.toml", spark_name))?;
        }
    }
    
    Ok(())
}

// Helper function to check if a spark is registered in registry.rs
fn check_spark_in_registry(spark_name: &str, project_dir: &Path) -> Result<bool, String> {
    let registry_path = project_dir.join("src").join("services").join("sparks").join("registry.rs");
    
    if !registry_path.exists() {
        return Ok(false);
    }
    
    let content = fs::read_to_string(&registry_path)
        .map_err(|e| format!("Failed to read registry.rs: {}", e))?;
    
    let pattern = format!("\"{spark_name}\" =>");
    Ok(content.contains(&pattern))
}

// Helper function to check if a spark is declared in mod.rs
fn check_spark_in_mod_rs(spark_name: &str, sparks_dir: &Path) -> Result<bool, String> {
    let mod_rs_path = sparks_dir.join("mod.rs");
    
    if !mod_rs_path.exists() {
        return Ok(false);
    }
    
    let content = fs::read_to_string(&mod_rs_path)
        .map_err(|e| format!("Failed to read mod.rs: {}", e))?;
    
    let module_line = format!("pub mod {};", spark_name);
    Ok(content.contains(&module_line))
}

// Helper function to extract all spark names from registry.rs
fn get_sparks_from_registry(project_dir: &Path) -> Result<Vec<String>, String> {
    let registry_path = project_dir.join("src").join("services").join("sparks").join("registry.rs");
    
    if !registry_path.exists() {
        return Ok(Vec::new());
    }
    
    let content = fs::read_to_string(&registry_path)
        .map_err(|e| format!("Failed to read registry.rs: {}", e))?;
    
    let mut spark_names = Vec::new();
    
    // Look for patterns like "spark_name" => {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('"') && trimmed.contains("\" =>") {
            // Extract spark name from "spark_name" => pattern
            if let Some(end_quote) = trimmed[1..].find('"') {
                let spark_name = &trimmed[1..end_quote + 1];
                // Skip the default wildcard case
                if spark_name != "_" {
                    spark_names.push(spark_name.to_string());
                }
            }
        }
    }
    
    Ok(spark_names)
}

// Helper function to repair spark registration without re-downloading
fn repair_spark_registration(spark_name: &str, sparks_dir: &Path, project_dir: &Path) -> Result<(), String> {
    logger::info(&format!("Repairing registration for spark: {}", spark_name))?;
    
    // Check and repair mod.rs
    if !check_spark_in_mod_rs(spark_name, sparks_dir)? {
        logger::info(&format!("Adding '{}' to mod.rs", spark_name))?;
        update_sparks_mod_rs(&sparks_dir.to_path_buf(), spark_name)?;
    }
    
    // Check and repair registry.rs  
    if !check_spark_in_registry(spark_name, project_dir)? {
        logger::info(&format!("Adding '{}' to registry.rs", spark_name))?;
        update_spark_registry(project_dir, spark_name)?;
    }
    
    logger::success(&format!("Successfully repaired spark registration: {}", spark_name))?;
    Ok(())
}

pub fn install_sparks_from_config(config: &Config) -> Result<(), String> {
    // Check if there's a sparks section in the config
    if let Some(sparks) = config.assets.get("sparks") {
        if let Some(sparks_table) = sparks.as_table() {
            if sparks_table.is_empty() {
                logger::info("No sparks defined in Catalyst.toml")?;
                return Ok(());
            }

            logger::info(&format!("Found {} spark(s) in Catalyst.toml", sparks_table.len()))?;

            // Setup a progress bar for installation
            let total_sparks = sparks_table.len() as u64;
            let mut progress = logger::create_progress(Some(total_sparks));
            progress.set_message("Installing sparks from Catalyst.toml...");

            let mut current = 0;
            for (spark_name, spark_url) in sparks_table {
                current += 1;
                if let Some(url) = spark_url.as_str() {
                    progress.set_message(&format!("Installing spark ({}/{}): {}", current, total_sparks, spark_name));

                    // Attempt to add the spark
                    if let Err(e) = add_spark(url, config) {
                        progress.warning(&format!("Failed to install spark {}: {}", spark_name, e))?;
                    } else {
                        progress.set_message(&format!("Installed spark ({}/{}): {}", current, total_sparks, spark_name));
                    }
                } else {
                    progress.warning(&format!("Invalid URL for spark: {}", spark_name))?;
                }
                progress.inc(1);
            }

            progress.success("Spark installation complete!");
        }
    } else {
        logger::info("No sparks section in Catalyst.toml")?;
    }

    Ok(())
}

// Helper function to update the sparks/mod.rs file to include the new spark
fn update_sparks_mod_rs(sparks_dir: &PathBuf, spark_name: &str) -> Result<(), String> {
    let mod_rs_path = sparks_dir.join("mod.rs");

    if !mod_rs_path.exists() {
        return Err(format!("mod.rs file not found in sparks directory"));
    }

    // Read the current content
    let mut content = fs::read_to_string(&mod_rs_path).map_err(|e| format!("Failed to read mod.rs file: {}", e))?;

    // Check if the module is already included
    let module_line = format!("pub mod {};", spark_name);
    if !content.contains(&module_line) {
        // Add the module line after the comment or at the end
        if content.contains("//here you include the modules you want to expose to the outside world") {
            content.push_str(&format!("\npub mod {};\n", spark_name));
        } else {
            content.push_str(&format!("\n//here you include the modules you want to expose to the outside world\npub mod {};\n", spark_name));
        }

        // Write the updated content
        fs::write(&mod_rs_path, content).map_err(|e| format!("Failed to update mod.rs file: {}", e))?;
    }

    Ok(())
}

// Helper function to update the registry.rs file to add a new match arm for the spark
pub fn update_spark_registry(project_dir: &Path, spark_name: &str) -> Result<(), String> {
    let registry_path = project_dir.join("src").join("services").join("sparks").join("registry.rs");

    // Check if registry.rs exists
    if !registry_path.exists() {
        logger::info("Spark registry.rs not found. This will be created automatically when the project is initialized.")?;
        return Ok(());
    }

    logger::info(&format!("Updating spark registry for: {}", spark_name))?;

    // Read the current content
    let content = fs::read_to_string(&registry_path).map_err(|e| format!("Failed to read registry.rs file: {}", e))?;

    // Check if the spark is already registered
    if content.contains(&format!("\"{spark_name}\" =>")) {
        logger::info(&format!("Spark '{}' is already registered in registry.rs", spark_name))?;
        return Ok(());
    }

    // Find the match block in the register_by_name function
    if let Some(match_start_pos) = content.find("match name {") {
        // Find the first default case after the match name statement
        if let Some(default_pos) = content[match_start_pos..].find("_ => {") {
            let absolute_default_pos = match_start_pos + default_pos;

            // Create the new match arm - we don't need to use the match content
            let new_match_arm = format!("\n        \"{spark_name}\" => {{\n            register_spark(name, {spark_name}::create_spark);\n            true\n        }},");

            // Construct the new content by inserting the match arm just before the default case
            let mut updated_content = content.clone();
            updated_content.insert_str(absolute_default_pos, &new_match_arm);

            // Write the updated content
            fs::write(&registry_path, updated_content).map_err(|e| format!("Failed to update registry.rs file: {}", e))?;

            logger::success(&format!("Updated registry.rs to include spark: {}", spark_name))?;
            return Ok(());
        }
    }

    logger::warning(&format!("Could not find register_by_name function in registry.rs. Manual update required."))?;
    Ok(())
}
