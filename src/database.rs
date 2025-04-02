use crate::progress::ProgressManager;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Select};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn migrate() -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Running database migrations...");

    // Check if migrations directory exists
    if !Path::new("src/database/migrations").exists() {
        progress.error("No migrations directory found. Skipping migration operation.");
        return false;
    }

    // Test database connection first
    if let Err(e) = establish_connection() {
        progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
        progress.error("Hint: Make sure PostgreSQL is running and accessible with the credentials in your .env file");
        return false;
    }

    // Run migration command
    let output = match Command::new("diesel").args(["migration", "run"]).stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
        Ok(output) => output,
        Err(e) => {
            progress.error(&format!("Error executing diesel migration run: {}", e));
            return false;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Extract migrations from stdout
    let migrations: Vec<String> = stdout
        .lines()
        .filter(|line| line.contains("Running migration"))
        .filter_map(|line| line.split("Running migration").nth(1).map(|name| name.trim().to_string()))
        .collect();

    let has_output = stdout.lines().next().is_some();
    let errors: Vec<String> = stderr.lines().map(|line| line.trim().to_string()).collect();
    let has_errors = !errors.is_empty();

    match (has_output, has_errors, migrations.is_empty()) {
        (false, false, _) => progress.success("No migrations to run"),
        (_, false, false) => progress.success(&format!("Ran {} migrations: {}", migrations.len(), migrations.join(", "))),
        (_, false, true) => progress.success("Migrations completed successfully"),
        (_, true, _) => {
            if !errors.is_empty() {
                progress.error(&format!("Migration errors: {}", errors.join(", ")));
            } else {
                progress.error("Some migrations failed");
            }
            return false;
        }
    }

    true
}

// Helper function to handle diesel command output for rollbacks
fn handle_diesel_output(output: &std::process::Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";
    let log_fn = |line: &str, success: bool| {
        let prefix = if success { "\x1b[32m✔\x1b[0m" } else { "\x1b[31m✖\x1b[0m" };
        let formatted_line = format!("{} {}", prefix, line);
        if is_interactive {
            let _ = crate::output::log(&formatted_line);
        } else {
            println!("{}", formatted_line);
        }
    };

    // Process stdout lines
    stdout.lines().for_each(|line| log_fn(line, true));

    // Process stderr lines and track if we found any errors
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

// Helper function to run diesel migration commands with common error handling
fn run_diesel_migration(args: &[&str], progress_msg: &str) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message(progress_msg);

    // Test database connection first
    if let Err(e) = establish_connection() {
        progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
        progress.error("Hint: Make sure PostgreSQL is running and accessible with the credentials in your .env file");
        return false;
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

#[allow(dead_code)]
pub fn rollback_one() -> bool {
    run_diesel_migration(&["migration", "revert"], "Rolling back one migration...")
}

pub fn rollback_all() -> bool {
    run_diesel_migration(&["migration", "revert", "--all"], "Rolling back all migrations...")
}

// Get a list of available connection names from the .env file
fn get_connection_names() -> Vec<String> {
    dotenv().ok();

    let mut names = Vec::new();
    names.push("default".to_string()); // Default connection is always available

    // Look for any DATABASE_URL_* variables
    for (key, _) in env::vars() {
        if key.starts_with("DATABASE_URL_") {
            let name = key.replace("DATABASE_URL_", "").to_lowercase();
            names.push(name);
        }
    }

    names
}

// Generate schema for a specific database connection
pub fn generate_schema_for_connection(conn_name: &str) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message(&format!("Generating schema for {} connection...", conn_name));

    // Make sure the directory exists
    if !Path::new("src/database").exists() {
        if let Err(e) = fs::create_dir_all("src/database") {
            progress.error(&format!("Error creating schema directory: {}", e));
            return false;
        }
    }

    // Determine the environment variable to use
    let env_var = if conn_name == "default" {
        "DATABASE_URL".to_string()
    } else {
        format!("DATABASE_URL_{}", conn_name.to_uppercase())
    };

    // Get the database URL
    let database_url = match env::var(&env_var) {
        Ok(url) => url,
        Err(_) => {
            progress.error(&format!("{} not found in environment", env_var));
            return false;
        }
    };

    // Determine the output file path
    let schema_file = if conn_name == "default" {
        "src/database/schema.rs".to_string()
    } else {
        format!("src/database/schema_{}.rs", conn_name.to_lowercase())
    };

    // Run diesel print-schema command with the appropriate DATABASE_URL
    let output = match Command::new("diesel").arg("print-schema").env("DATABASE_URL", &database_url).stdout(Stdio::piped()).spawn() {
        Ok(child) => match child.wait_with_output() {
            Ok(output) => output,
            Err(e) => {
                progress.error(&format!("Error executing diesel print-schema: {}", e));
                return false;
            }
        },
        Err(e) => {
            progress.error(&format!("Error spawning diesel print-schema: {}", e));
            return false;
        }
    };

    if !output.status.success() {
        progress.error("diesel print-schema command failed");
        return false;
    }

    let schema_str = String::from_utf8_lossy(&output.stdout);

    // Create the schema file
    match File::create(&schema_file) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(schema_str.as_bytes()) {
                progress.error(&format!("Error writing schema file: {}", e));
                false
            } else {
                // Count number of tables in the schema
                let table_count = schema_str.matches("table!").count();
                progress.success(&format!("Generated schema for {} with {} tables", conn_name, table_count));
                true
            }
        }
        Err(e) => {
            progress.error(&format!("Error creating schema file: {}", e));
            false
        }
    }
}

// Generate schemas for all databases in the .env file
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

    // Update the mod.rs file to include all schemas
    update_schema_mod_file(&generated_connections);

    success
}

// Update the mod.rs file to include all schemas
fn update_schema_mod_file(connections: &[String]) {
    let mod_path = "src/database/mod.rs";

    // First, read the existing file if any
    let existing_content = fs::read_to_string(mod_path).unwrap_or_default();

    // Extract any non-schema module declarations
    let other_modules: Vec<String> = existing_content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("pub mod ") && !trimmed.starts_with("pub mod schema") && !trimmed.contains("schema_")
        })
        .map(|line| line.to_string())
        .collect();

    // Build new content with schema declarations first
    let mut content = String::new();
    let mut added_modules = std::collections::HashSet::new();

    // First add default schema
    if connections.contains(&"default".to_string()) {
        content.push_str("pub mod schema;\n");
        added_modules.insert("schema".to_string());
    }

    // Then add other schemas
    connections.iter().filter(|&conn_name| conn_name != "default").for_each(|conn_name| {
        let module_name = format!("schema_{}", conn_name);
        if !added_modules.contains(&module_name) {
            content.push_str(&format!("pub mod {};\n", module_name));
            added_modules.insert(module_name);
        }
    });

    // Add other modules after schema declarations
    other_modules.iter().for_each(|module| {
        content.push_str(&format!("{}\n", module));
    });

    // Write the mod.rs file
    if let Err(e) = fs::write(mod_path, content) {
        eprintln!("Error updating schema mod.rs file: {}", e);
    }

    // Now update the db.rs file with connection functions for each database
    update_db_connection_functions(connections);
}

// Generate simple connection functions in db.rs for each database
fn update_db_connection_functions(connections: &[String]) {
    let db_path = "src/database/db.rs";

    // Try to read the existing db.rs file
    if let Ok(existing_content) = fs::read_to_string(db_path) {
        // Extract the base part (up to the comment about additional functions)
        let base_parts: Vec<&str> = existing_content.split("// Additional connection functions").collect();

        let mut new_content = if base_parts.len() > 1 {
            // If we found the marker comment, use everything before it
            base_parts[0].to_string()
        } else {
            // Otherwise use the whole file
            existing_content.clone()
        };

        // Add the marker comment
        new_content.push_str("// Additional connection functions will be generated by the blast tool\n");
        new_content.push_str("// based on DATABASE_URL_* entries in the .env file\n");

        // Add connection functions for each additional database
        for conn_name in connections {
            if conn_name != "default" {
                let func_name = format!("establish_connection_{}", conn_name);
                let env_var = format!("DATABASE_URL_{}", conn_name.to_uppercase());

                new_content.push_str(&format!(
                    r#"
pub fn {}() -> PgConnection {{
    dotenv().ok();
    let database_url = env::var("{}").expect("{} must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {{}}", database_url))
}}
"#,
                    func_name, env_var, env_var
                ));
            }
        }

        // Write the updated db.rs file
        if let Err(e) = fs::write(db_path, new_content) {
            eprintln!("Error updating db.rs file: {}", e);
        }
    }
}

pub fn generate_schema() -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating database schema...");

    // Make sure the directory exists
    if !Path::new("src/database").exists() {
        if let Err(e) = fs::create_dir_all("src/database") {
            progress.error(&format!("Error creating schema directory: {}", e));
            return false;
        }
    }

    // Test database connection first
    match establish_connection() {
        Ok(_) => {
            // Connection successful, continue with schema generation
        }
        Err(e) => {
            progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
            progress.error("Hint: Make sure PostgreSQL is running and accessible with the credentials in your .env file");
            return false;
        }
    }

    // Check if we should generate multiple schemas
    let connections = get_connection_names();
    if connections.len() > 1 {
        // Ask user if they want to generate schema for all connections
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Found {} database connections. Generate schema for:", connections.len()))
            .items(&["Default database only", "All database connections"])
            .default(0)
            .interact();

        match selection {
            Ok(0) => {
                // Generate just the default schema
                return generate_schema_for_connection("default");
            }
            Ok(1) => {
                // Generate schema for all connections
                return generate_all_schemas();
            }
            _ => {
                progress.error("Schema generation cancelled");
                return false;
            }
        }
    }

    // If there's only one connection, just generate for default
    generate_schema_for_connection("default")
}

fn get_existing_tables() -> Vec<String> {
    let migrations_dir = Path::new("src/database/migrations");
    let mut tables_set = HashSet::new();

    if migrations_dir.exists() {
        for entry in fs::read_dir(migrations_dir).expect("Failed to read migrations directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip directories like diesel_initial_setup
                if path.file_name().is_some_and(|name| name.to_str().unwrap().contains("diesel_initial_setup")) {
                    continue;
                }

                let up_file_path = path.join("up.sql");
                if up_file_path.exists() {
                    if let Ok(contents) = fs::read_to_string(&up_file_path) {
                        for line in contents.lines() {
                            let line = line.trim();
                            if line.to_uppercase().starts_with("CREATE TABLE") {
                                // Find the position of "CREATE TABLE" and extract the rest
                                let create_table_pos = line.find("TABLE").unwrap() + "TABLE".len();
                                let rest_of_line = &line[create_table_pos..].trim();

                                // Handle possible "IF NOT EXISTS" and schema prefixes
                                let table_name_part = rest_of_line.strip_prefix("IF NOT EXISTS").unwrap_or(rest_of_line).trim();

                                // Split by whitespace or period(.) and take the last part
                                let table_name_candidate = table_name_part.split_whitespace().next().map(|name| name.split('.').last().unwrap_or(name));

                                if let Some(table_name) = table_name_candidate {
                                    // Clean the table name from any unwanted characters
                                    let clean_table_name = table_name.trim_matches(|c| c == '(' || c == '`' || c == ';' || c == '"');
                                    tables_set.insert(clean_table_name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut tables: Vec<String> = tables_set.into_iter().collect();
    tables.sort(); // Sort the table names for consistent display
    tables
}

pub fn new_migration() {
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";
    let log_message = |msg: &str| {
        if is_interactive {
            let _ = crate::output::log(msg);
        } else {
            println!("{}", msg);
        }
    };

    let theme = ColorfulTheme::default();
    let multi_progress = MultiProgress::new();
    let spinner_style = ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap();

    // Main spinner for overall progress
    let main_spinner = multi_progress.add(ProgressBar::new_spinner());
    main_spinner.set_style(spinner_style.clone());
    main_spinner.set_message("Creating new migration...");

    // Helper function to create a styled selection prompt
    let create_select = |prompt: &str, items: Vec<&str>, default: usize| Select::with_theme(&theme).with_prompt(prompt).default(default).items(&items);

    // Step 1: Choose migration type
    let mut step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message("Step 1: Choose migration type");

    let actions = vec!["Create New Table", "Alter Existing Table", "Custom Migration", "🔙 Cancel"];
    let mut current_step = 1;
    let max_steps_by_type = [5, 5, 3]; // [new table, alter table, custom]

    let action_result = create_select("What type of migration do you want to create?", actions, 0).interact();

    let action = match action_result {
        Ok(index) => index,
        Err(_) => {
            log_message("Migration creation cancelled");
            return;
        }
    };

    if action == 3 {
        // Cancel option
        main_spinner.finish_with_message("Migration creation cancelled");
        return;
    }

    // Setup data for the migration
    let is_new_table;
    let is_custom_migration;
    let mut table_name = String::new();
    let max_step = max_steps_by_type[action];

    match action {
        0 => {
            is_new_table = true;
            is_custom_migration = false;
        }
        1 => {
            is_new_table = false;
            is_custom_migration = false;
        }
        2 => {
            is_new_table = false;
            is_custom_migration = true;
        }
        _ => {
            log_message("Migration creation cancelled");
            return;
        }
    }

    step_spinner.finish_and_clear();

    // Step 2: Get table information
    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());

    step_spinner.set_message(format!("Step {}/{}: {}", current_step, max_step, if is_custom_migration { "Migration name" } else { "Table information" }));

    // Different flow for custom migrations
    let migration_name: String;
    if is_custom_migration {
        let input_result = Input::with_theme(&theme).with_prompt("Enter a name for your custom migration").interact_text();

        match input_result {
            Ok(name) => {
                migration_name = name;
            }
            Err(_) => {
                main_spinner.finish_with_message("Migration creation cancelled");
                return;
            }
        }
    } else {
        // For table migrations, get table name
        if is_new_table {
            let input_result = Input::with_theme(&theme).with_prompt("Enter the new table name").interact_text();

            match input_result {
                Ok(name) => {
                    table_name = name;
                    migration_name = format!("create_{}", table_name);
                }
                Err(_) => {
                    main_spinner.finish_with_message("Migration creation cancelled");
                    return;
                }
            }
        } else {
            // For alter table, select existing table
            let existing_tables = get_existing_tables();
            if existing_tables.is_empty() {
                log_message("No existing tables found. You must create a new table first.");
                main_spinner.finish_with_message("Migration creation cancelled - no tables found");
                return;
            }

            // Add back option to the table selection
            let mut table_choices: Vec<String> = existing_tables.clone();
            table_choices.push("🔙 Go back".to_string());

            let select_result = FuzzySelect::with_theme(&theme).with_prompt("Select a table to alter").items(&table_choices).default(0).interact();

            match select_result {
                Ok(index) => {
                    if index == table_choices.len() - 1 {
                        // User selected "Go back"
                        new_migration();
                        return;
                    }

                    table_name = existing_tables[index].clone();
                    migration_name = format!("alter_{}", table_name);
                }
                Err(_) => {
                    main_spinner.finish_with_message("Migration creation cancelled");
                    return;
                }
            }
        }
    }

    step_spinner.finish_and_clear();

    let mut columns = Vec::new();
    let mut foreign_keys = Vec::new();

    // For custom migrations, skip the column definition steps
    if is_custom_migration {
        // Step 3 for custom migrations: Generate migration files
        current_step += 1;
        step_spinner = multi_progress.add(ProgressBar::new_spinner());
        step_spinner.set_style(spinner_style.clone());
        step_spinner.set_message(format!("Step {}/{}: Creating migration files", current_step, max_step));

        // Call Diesel CLI to create a new migration
        let output = Command::new("diesel").args(["migration", "generate", &migration_name]).output().expect("Failed to execute Diesel command");

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            log_message(&format!("Failed to generate migration: {}", error));
            main_spinner.finish_with_message("Migration creation failed");
            return;
        }

        // Parse the output to get the up and down file paths
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout_str.lines().collect();

        if lines.len() < 2 {
            log_message("Unexpected output format from Diesel command.");
            main_spinner.finish_with_message("Migration creation failed");
            return;
        }

        let up_file = lines[0].trim().replace("Creating ", "");
        let down_file = lines[1].trim().replace("Creating ", "");

        // Write some helpful comments to the up.sql file
        let up_sql = "-- Write your custom SQL migration here\n-- Example: ALTER TABLE table_name ADD COLUMN column_name TYPE;\n";
        let down_sql = "-- Write how to reverse the changes here\n-- Example: ALTER TABLE table_name DROP COLUMN column_name;\n";

        fs::write(&up_file, up_sql).expect("Unable to write up.sql");
        fs::write(&down_file, down_sql).expect("Unable to write down.sql");

        main_spinner.finish_with_message(format!("✅ Custom migration '{}' created successfully!", migration_name));
        log_message(&format!("Migration files created at:\n- {}\n- {}", up_file, down_file));
        log_message("Edit these files with your custom SQL migrations.");

        return;
    }

    // If it's a new table, automatically add the id column
    if is_new_table {
        columns.push((
            "id".to_string(),
            "SERIAL".to_string(),
            true,          // NOT NULL
            true,          // UNIQUE
            String::new(), // No default value
            true,          // PRIMARY KEY
        ));

        log_message(&format!("Automatically added 'id SERIAL PRIMARY KEY' to new table '{}'.", table_name));
    }

    // Step 3: Column Definition
    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message(format!("Step {}/{}: Column definition", current_step, max_step));

    let column_types = vec![
        "SERIAL",
        "INTEGER",
        "BIGINT",
        "SMALLINT",
        "VARCHAR",
        "TEXT",
        "CHAR",
        "BOOLEAN",
        "FLOAT",
        "DOUBLE PRECISION",
        "DECIMAL",
        "NUMERIC",
        "DATE",
        "TIME",
        "TIMESTAMP",
        "TIMESTAMPTZ",
        "UUID",
        "JSON",
        "JSONB",
        "ARRAY",
    ];

    // Column definition loop
    loop {
        let items = vec!["Add column", "Continue to next step", "🔙 Go back"];
        let action_select = create_select(&format!("Columns defined: {}. What would you like to do?", columns.len()), items, 0).interact();

        let column_action = match action_select {
            Ok(index) => index,
            Err(_) => {
                main_spinner.finish_with_message("Migration creation cancelled");
                return;
            }
        };

        match column_action {
            0 => {
                // Add a new column
                let column_name_result = Input::with_theme(&theme).with_prompt("Enter column name").interact_text();

                let column_name: String = match column_name_result {
                    Ok(name) => name,
                    Err(_) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };

                let type_select = FuzzySelect::with_theme(&theme)
                    .with_prompt(&format!("Select type for column '{}'", column_name))
                    .items(&column_types)
                    .default(0)
                    .interact();

                let type_index = match type_select {
                    Ok(index) => index,
                    Err(_) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };

                let mut column_type = column_types[type_index].to_string();

                // Handle type-specific parameters
                if column_type == "VARCHAR" || column_type == "CHAR" {
                    let length_result = Input::<usize>::with_theme(&theme).with_prompt(&format!("Enter length for {}", column_type)).default(255).interact_text();

                    let length = match length_result {
                        Ok(len) => len,
                        Err(_) => {
                            main_spinner.finish_with_message("Migration creation cancelled");
                            return;
                        }
                    };

                    column_type = format!("{}({})", column_type, length);
                } else if column_type == "DECIMAL" || column_type == "NUMERIC" {
                    let precision_result = Input::<usize>::with_theme(&theme).with_prompt("Enter precision (total digits)").default(10).interact_text();

                    let scale_result = Input::<usize>::with_theme(&theme).with_prompt("Enter scale (decimal digits)").default(2).interact_text();

                    let precision = precision_result.unwrap_or(10);
                    let scale = scale_result.unwrap_or(2);

                    column_type = format!("{}({},{})", column_type, precision, scale);
                } else if column_type == "ARRAY" {
                    let elem_type_select = FuzzySelect::with_theme(&theme)
                        .with_prompt("Select the array element type")
                        .items(&["INTEGER", "TEXT", "VARCHAR", "BOOLEAN", "FLOAT", "UUID"])
                        .default(0)
                        .interact();

                    let elem_type_index = elem_type_select.unwrap_or(0);
                    let elem_type = match elem_type_index {
                        0 => "INTEGER",
                        1 => "TEXT",
                        2 => "VARCHAR",
                        3 => "BOOLEAN",
                        4 => "FLOAT",
                        5 => "UUID",
                        _ => "TEXT",
                    };

                    column_type = format!("{}[]", elem_type);
                }

                // Column properties
                let nullable_result = Confirm::with_theme(&theme).with_prompt("Is this column nullable?").default(false).interact();

                let nullable = nullable_result.unwrap_or(false);

                let unique_result = Confirm::with_theme(&theme).with_prompt("Is this column unique?").default(false).interact();

                let unique = unique_result.unwrap_or(false);

                let default_value_result = Input::<String>::with_theme(&theme).with_prompt("Enter default value (or leave empty for none)").allow_empty(true).interact_text();

                let default_value = default_value_result.unwrap_or_default();
                let default_value_display = if default_value.is_empty() { String::new() } else { format!("DEFAULT {} ", default_value) };

                let is_primary_key_result = if column_type == "SERIAL" {
                    Ok(true) // SERIAL columns are typically primary keys
                } else {
                    Confirm::with_theme(&theme).with_prompt("Is this column a primary key?").default(false).interact()
                };

                let is_primary_key = is_primary_key_result.unwrap_or(false);

                // Foreign key check
                let is_foreign_key_result = Confirm::with_theme(&theme).with_prompt("Is this column a foreign key?").default(false).interact();

                let is_foreign_key = is_foreign_key_result.unwrap_or(false);

                if is_foreign_key {
                    let existing_tables = get_existing_tables();
                    if existing_tables.is_empty() {
                        log_message("No existing tables found for foreign key reference.");
                    } else {
                        let ref_table_select = Select::with_theme(&theme).with_prompt("Select referenced table").items(&existing_tables).interact();

                        match ref_table_select {
                            Ok(index) => {
                                let referenced_table = existing_tables[index].clone();

                                // For simplicity, we'll assume the referenced column is "id"
                                let ref_column_result = Input::<String>::with_theme(&theme).with_prompt("Enter referenced column").default("id".to_string()).interact_text();

                                let referenced_column = ref_column_result.unwrap_or_else(|_| "id".to_string());

                                foreign_keys.push((column_name.clone(), referenced_table.clone(), referenced_column.clone()));

                                log_message(&format!("Added foreign key: {} references {}({})", column_name, referenced_table, referenced_column));
                            }
                            Err(_) => {
                                log_message("Foreign key creation cancelled.");
                            }
                        }
                    }
                }

                columns.push((column_name.clone(), column_type.clone(), nullable, unique, default_value, is_primary_key));

                log_message(&format!(
                    "Added column: {} {} {}{}{}{}",
                    column_name,
                    column_type,
                    if nullable { "" } else { "NOT NULL " },
                    if unique { "UNIQUE " } else { "" },
                    default_value_display,
                    if is_primary_key { "PRIMARY KEY" } else { "" }
                ));
            }
            1 => {
                // Continue to the next step
                break;
            }
            2 => {
                // Go back to the previous step
                new_migration();
                return;
            }
            _ => break,
        }
    }

    step_spinner.finish_and_clear();

    // Step 4: Review migration
    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message(format!("Step {}/{}: Review migration", current_step, max_step));

    // Build preview of migration SQL
    let mut up_sql_preview = if is_new_table {
        format!("CREATE TABLE IF NOT EXISTS {} (\n", table_name)
    } else {
        format!("ALTER TABLE {} ", table_name)
    };

    if is_new_table {
        for (i, (name, typ, nullable, unique, ref default, is_primary_key)) in columns.iter().enumerate() {
            up_sql_preview.push_str(&format!(
                "    {} {}{}{}{}{}", // No trailing comma yet
                name,
                typ,
                if *nullable { "" } else { " NOT NULL" },
                if *unique { " UNIQUE" } else { "" },
                if default.is_empty() { String::new() } else { format!(" DEFAULT {}", default) },
                if *is_primary_key { " PRIMARY KEY" } else { "" }
            ));

            // Add comma if not the last item or if we have foreign keys
            if i < columns.len() - 1 || !foreign_keys.is_empty() {
                up_sql_preview.push_str(",\n");
            } else {
                up_sql_preview.push_str("\n");
            }
        }

        for (i, (column, ref_table, ref_column)) in foreign_keys.iter().enumerate() {
            up_sql_preview.push_str(&format!("    FOREIGN KEY ({}) REFERENCES {}({})", column, ref_table, ref_column));

            if i < foreign_keys.len() - 1 {
                up_sql_preview.push_str(",\n");
            } else {
                up_sql_preview.push_str("\n");
            }
        }

        up_sql_preview.push_str(");\n");
    } else {
        for (i, (name, typ, nullable, unique, ref default, _)) in columns.iter().enumerate() {
            up_sql_preview.push_str(&format!(
                "ADD COLUMN {} {}{}{}{}",
                name,
                typ,
                if *nullable { "" } else { " NOT NULL" },
                if *unique { " UNIQUE" } else { "" },
                if default.is_empty() { String::new() } else { format!(" DEFAULT {}", default) },
            ));

            if i < columns.len() - 1 || !foreign_keys.is_empty() {
                up_sql_preview.push_str(", ");
            }
        }

        for (i, (column, ref_table, ref_column)) in foreign_keys.iter().enumerate() {
            up_sql_preview.push_str(&format!("ADD FOREIGN KEY ({}) REFERENCES {}({})", column, ref_table, ref_column));

            if i < foreign_keys.len() - 1 {
                up_sql_preview.push_str(", ");
            }
        }

        up_sql_preview.push_str(";\n");
    }

    let down_sql_preview = if is_new_table { format!("DROP TABLE {};\n", table_name) } else { "-- reverse changes here\n".to_string() };

    // Show preview of the migration
    log_message("\n=== Migration Preview ===");
    log_message(&format!("Table: {}", table_name));
    log_message(&format!("Type: {}", if is_new_table { "Create new table" } else { "Alter existing table" }));
    log_message("\nUp SQL:");
    log_message(&up_sql_preview);
    log_message("\nDown SQL:");
    log_message(&down_sql_preview);
    log_message("======================\n");

    // Confirm creation
    let confirm_result = Select::with_theme(&theme)
        .with_prompt("How would you like to proceed?")
        .items(&["Create migration", "Edit migration", "🔙 Go back", "❌ Cancel"])
        .default(0)
        .interact();

    let confirm_action = match confirm_result {
        Ok(index) => index,
        Err(_) => {
            main_spinner.finish_with_message("Migration creation cancelled");
            return;
        }
    };

    match confirm_action {
        0 => {
            // Proceed with creation
        }
        1 => {
            // Edit migration - restart from beginning
            new_migration();
            return;
        }
        2 => {
            // Go back to start
            new_migration();
            return;
        }
        _ => {
            // Cancel
            main_spinner.finish_with_message("Migration creation cancelled");
            return;
        }
    };

    step_spinner.finish_and_clear();

    // Step 5: Create migration files
    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message(format!("Step {}/{}: Creating migration files", current_step, max_step));

    // Call Diesel CLI to create a new migration
    let migration_type = if is_new_table { "create" } else { "alter" };
    let output = Command::new("diesel")
        .args(["migration", "generate", &format!("{}_{}", migration_type, table_name)])
        .output()
        .expect("Failed to execute Diesel command");

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        log_message(&format!("Failed to generate migration: {}", error));
        main_spinner.finish_with_message("Migration creation failed");
        return;
    }

    // Parse the output to get the up and down file paths
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout_str.lines().collect();

    if lines.len() < 2 {
        log_message("Unexpected output format from Diesel command.");
        main_spinner.finish_with_message("Migration creation failed");
        return;
    }

    let up_file = lines[0].trim().replace("Creating ", "");
    let down_file = lines[1].trim().replace("Creating ", "");

    // Write the SQL to the migration files
    fs::write(&up_file, up_sql_preview).expect("Unable to write up.sql");
    fs::write(&down_file, down_sql_preview).expect("Unable to write down.sql");

    step_spinner.finish_and_clear();

    main_spinner.finish_with_message(format!("✅ Migration for table '{}' created successfully!", table_name));

    log_message(&format!("Migration files created at:\n- {}\n- {}", up_file, down_file));
}

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

// Function to seed a specific file by name
pub fn seed_specific_file(file_name: &str) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message(&format!("Running seed file {}", file_name));

    // Try to establish a database connection first
    let connection_result = establish_connection();
    let mut connection = match connection_result {
        Ok(conn) => conn,
        Err(e) => {
            progress.error(&format!("Database connection failed: {}. Is PostgreSQL running?", e));
            return false;
        }
    };

    // Check if file exists
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

pub fn seed(selection: Option<usize>) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Running database seed operations...");

    // Try to establish a database connection first
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

    // Get and sort seed files
    let seed_files = match fs::read_dir(seed_dir) {
        Ok(entries) => {
            let files: Vec<String> = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_file())
                .filter_map(|entry| entry.path().file_name().map(|name| name.to_string_lossy().into_owned()))
                .collect();

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

    // Handle batch mode vs interactive mode
    if let Some(_) = selection {
        // Run all seed files in batch mode
        return run_all_seed_files(&mut connection, seed_files);
    }

    // Interactive mode
    let items: Vec<&str> = std::iter::once("All").chain(seed_files.iter().map(|s| s.as_str())).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a seed file to run or choose All")
        .default(0)
        .items(&items)
        .interact()
        .unwrap();

    if selection == 0 {
        // Run all seed files
        run_all_seed_files(&mut connection, seed_files)
    } else {
        // Run selected seed file
        let file = &seed_files[selection - 1];
        let seed_progress = ProgressManager::new_spinner();
        seed_progress.set_message(&format!("Seeding {}", file));

        let result = run_seed_file(&mut connection, file);
        if result {
            seed_progress.success(&format!("Seed file {} executed successfully", file));
        } else {
            seed_progress.error(&format!("Failed to execute seed file {}", file));
        }

        result
    }
}

// Helper function to run all seed files
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

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    // Read seed file
    let sql = match fs::read_to_string(&seed_path) {
        Ok(content) => content,
        Err(e) => {
            // Don't display individual progress messages here
            // The calling function will collect and display errors
            let error_msg = format!("Error: Unable to read seed file {}: {}", file_name, e);

            if is_interactive {
                // In interactive mode, log to file
                let _ = crate::output::log(&error_msg);
            } else {
                // In CLI mode, print to stderr
                eprintln!("{}", error_msg);
            }
            return false;
        }
    };

    // Execute seed SQL
    match diesel::sql_query(sql).execute(connection) {
        Ok(_) => {
            // Success without output
            true
        }
        Err(e) => {
            // Error without output
            let error_msg = format!("Error: Failed to execute seed file {}: {}", file_name, e);

            if is_interactive {
                // In interactive mode, log to file
                let _ = crate::output::log(&error_msg);
            } else {
                // In CLI mode, print to stderr
                eprintln!("{}", error_msg);
            }
            false
        }
    }
}

fn establish_connection() -> Result<PgConnection, Box<dyn std::error::Error>> {
    dotenv().ok();

    // Check if PostgreSQL is installed
    let postgres_available = Command::new("which").arg("psql").output().map(|output| output.status.success()).unwrap_or(false);

    // Get database URL from environment
    let database_url = env::var("DATABASE_URL").map_err(|_| {
        let suggestion = if postgres_available {
            "DATABASE_URL environment variable not set. Make sure you have a .env file with DATABASE_URL=postgres://username:password@localhost/dbname"
        } else {
            "DATABASE_URL environment variable not set and PostgreSQL might not be installed. \
            Please install PostgreSQL and create a .env file with DATABASE_URL=postgres://username:password@localhost/dbname"
        };

        Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, suggestion)) as Box<dyn std::error::Error>
    })?;

    // Try to establish connection
    PgConnection::establish(&database_url).map_err(|e| {
        // Check if service is running
        let service_running = Command::new("pg_isready").args(["-h", "localhost"]).output().map(|output| output.status.success()).unwrap_or(false);

        let error_message = format!("Could not connect to database via `{}`: {}", database_url, e);
        let suggestion = if !service_running {
            format!("{}. PostgreSQL service appears to be down. Try starting it with: sudo service postgresql start", error_message)
        } else {
            format!("{}. PostgreSQL is running but connection failed. Check your credentials and database existence", error_message)
        };

        Box::new(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, suggestion)) as Box<dyn std::error::Error>
    })
}
