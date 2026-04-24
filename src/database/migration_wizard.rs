use crate::database::migrations::ensure_diesel_postgres;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Select};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::process::Command;

pub fn new_migration() {
    ensure_diesel_postgres();

    let is_interactive = match std::env::var("BLAST_INTERACTIVE") {
        Ok(v) => v == "1",
        Err(e) => {
            drop(e);
            false
        }
    };

    let log_message = |msg: &str| {
        if is_interactive {
            if let Err(e) = crate::output::log(msg) {
                drop(e);
            }
        } else {
            println!("{}", msg);
        }
    };

    let theme = ColorfulTheme::default();
    let multi_progress = MultiProgress::new();
    let spinner_style = match ProgressStyle::default_spinner().template("{spinner:.green} {msg}") {
        Ok(s) => s,
        Err(e) => {
            log_message(&format!("Failed to create spinner style: {}", e));
            return;
        }
    };

    let main_spinner = multi_progress.add(ProgressBar::new_spinner());
    main_spinner.set_style(spinner_style.clone());
    main_spinner.set_message("Creating new migration...");

    let create_select = |prompt: &str, items: Vec<&str>, default: usize| {
        Select::with_theme(&theme)
            .with_prompt(prompt)
            .default(default)
            .items(&items)
    };

    let mut step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message("Step 1: Choose migration type");

    let actions = vec!["Create New Table", "Alter Existing Table", "Custom Migration", "🔙 Cancel"];
    let mut current_step = 1;
    let max_steps_by_type = [5, 5, 3];

    let action = match create_select("What type of migration do you want to create?", actions, 0)
        .interact()
    {
        Ok(index) => index,
        Err(_e) => {
            log_message("Migration creation cancelled");
            return;
        }
    };

    if action == 3 {
        main_spinner.finish_with_message("Migration creation cancelled");
        return;
    }

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
        _other => {
            log_message("Migration creation cancelled");
            return;
        }
    }

    step_spinner.finish_and_clear();

    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message(format!(
        "Step {}/{}: {}",
        current_step,
        max_step,
        if is_custom_migration { "Migration name" } else { "Table information" }
    ));

    let migration_name: String;
    if is_custom_migration {
        migration_name = match Input::with_theme(&theme)
            .with_prompt("Enter a name for your custom migration")
            .interact_text()
        {
            Ok(name) => name,
            Err(_e) => {
                main_spinner.finish_with_message("Migration creation cancelled");
                return;
            }
        };
    } else if is_new_table {
        match Input::with_theme(&theme)
            .with_prompt("Enter the new table name")
            .interact_text()
        {
            Ok(name) => {
                table_name = name;
                migration_name = format!("create_{}", table_name);
            }
            Err(_e) => {
                main_spinner.finish_with_message("Migration creation cancelled");
                return;
            }
        }
    } else {
        let existing_tables = crate::database::seeds::get_existing_tables();
        if existing_tables.is_empty() {
            log_message("No existing tables found. You must create a new table first.");
            main_spinner.finish_with_message("Migration creation cancelled - no tables found");
            return;
        }

        let mut table_choices: Vec<String> = existing_tables.clone();
        table_choices.push("🔙 Go back".to_string());

        match FuzzySelect::with_theme(&theme)
            .with_prompt("Select a table to alter")
            .items(&table_choices)
            .default(0)
            .interact()
        {
            Ok(index) => {
                if index == table_choices.len() - 1 {
                    new_migration();
                    return;
                }
                table_name = existing_tables[index].clone();
                migration_name = format!("alter_{}", table_name);
            }
            Err(_e) => {
                main_spinner.finish_with_message("Migration creation cancelled");
                return;
            }
        }
    }

    step_spinner.finish_and_clear();

    let mut columns: Vec<(String, String, bool, bool, String, bool)> = Vec::new();
    let mut foreign_keys: Vec<(String, String, String)> = Vec::new();

    if is_custom_migration {
        current_step += 1;
        step_spinner = multi_progress.add(ProgressBar::new_spinner());
        step_spinner.set_style(spinner_style.clone());
        step_spinner.set_message(format!("Step {}/{}: Creating migration files", current_step, max_step));

        let output = match Command::new("diesel")
            .args(["migration", "generate", &migration_name])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                log_message(&format!("Failed to execute Diesel command: {}", e));
                main_spinner.finish_with_message("Migration creation failed");
                return;
            }
        };

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            log_message(&format!("Failed to generate migration: {}", error));
            main_spinner.finish_with_message("Migration creation failed");
            return;
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout_str.lines().collect();

        if lines.len() < 2 {
            log_message("Unexpected output format from Diesel command.");
            main_spinner.finish_with_message("Migration creation failed");
            return;
        }

        let up_file = lines[0].trim().replace("Creating ", "");
        let down_file = lines[1].trim().replace("Creating ", "");

        let up_sql = "-- Write your custom SQL migration here\n-- Example: ALTER TABLE table_name ADD COLUMN column_name TYPE;\n";
        let down_sql = "-- Write how to reverse the changes here\n-- Example: ALTER TABLE table_name DROP COLUMN column_name;\n";

        match fs::write(&up_file, up_sql) {
            Ok(()) => {}
            Err(e) => {
                log_message(&format!("Unable to write up.sql: {}", e));
                main_spinner.finish_with_message("Migration creation failed");
                return;
            }
        }
        match fs::write(&down_file, down_sql) {
            Ok(()) => {}
            Err(e) => {
                log_message(&format!("Unable to write down.sql: {}", e));
                main_spinner.finish_with_message("Migration creation failed");
                return;
            }
        }

        main_spinner.finish_with_message(format!("✅ Custom migration '{}' created successfully!", migration_name));
        log_message(&format!("Migration files created at:\n- {}\n- {}", up_file, down_file));
        log_message("Edit these files with your custom SQL migrations.");
        return;
    }

    if is_new_table {
        columns.push((
            "id".to_string(),
            "SERIAL".to_string(),
            true,
            true,
            String::new(),
            true,
        ));
        log_message(&format!("Automatically added 'id SERIAL PRIMARY KEY' to new table '{}'.", table_name));
    }

    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message(format!("Step {}/{}: Column definition", current_step, max_step));

    let column_types = vec![
        "SERIAL", "INTEGER", "BIGINT", "SMALLINT", "VARCHAR", "TEXT", "CHAR", "BOOLEAN",
        "FLOAT", "DOUBLE PRECISION", "DECIMAL", "NUMERIC", "DATE", "TIME", "TIMESTAMP",
        "TIMESTAMPTZ", "UUID", "JSON", "JSONB", "ARRAY",
    ];

    loop {
        let items = vec!["Add column", "Continue to next step", "🔙 Go back"];
        let column_action = match create_select(
            &format!("Columns defined: {}. What would you like to do?", columns.len()),
            items,
            0,
        )
        .interact()
        {
            Ok(index) => index,
            Err(_e) => {
                main_spinner.finish_with_message("Migration creation cancelled");
                return;
            }
        };

        match column_action {
            0 => {
                let column_name: String = match Input::with_theme(&theme)
                    .with_prompt("Enter column name")
                    .interact_text()
                {
                    Ok(name) => name,
                    Err(_e) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };

                let type_index = match FuzzySelect::with_theme(&theme)
                    .with_prompt(&format!("Select type for column '{}'", column_name))
                    .items(&column_types)
                    .default(0)
                    .interact()
                {
                    Ok(index) => index,
                    Err(_e) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };

                let mut column_type = column_types[type_index].to_string();

                if column_type == "VARCHAR" || column_type == "CHAR" {
                    let length = match Input::<usize>::with_theme(&theme)
                        .with_prompt(&format!("Enter length for {}", column_type))
                        .default(255)
                        .interact_text()
                    {
                        Ok(len) => len,
                        Err(_e) => {
                            main_spinner.finish_with_message("Migration creation cancelled");
                            return;
                        }
                    };
                    column_type = format!("{}({})", column_type, length);
                } else if column_type == "DECIMAL" || column_type == "NUMERIC" {
                    let precision = match Input::<usize>::with_theme(&theme)
                        .with_prompt("Enter precision (total digits)")
                        .default(10)
                        .interact_text()
                    {
                        Ok(v) => v,
                        Err(_e) => {
                            main_spinner.finish_with_message("Migration creation cancelled");
                            return;
                        }
                    };
                    let scale = match Input::<usize>::with_theme(&theme)
                        .with_prompt("Enter scale (decimal digits)")
                        .default(2)
                        .interact_text()
                    {
                        Ok(v) => v,
                        Err(_e) => {
                            main_spinner.finish_with_message("Migration creation cancelled");
                            return;
                        }
                    };
                    column_type = format!("{}({},{})", column_type, precision, scale);
                } else if column_type == "ARRAY" {
                    let elem_type_index = match FuzzySelect::with_theme(&theme)
                        .with_prompt("Select the array element type")
                        .items(&["INTEGER", "TEXT", "VARCHAR", "BOOLEAN", "FLOAT", "UUID"])
                        .default(0)
                        .interact()
                    {
                        Ok(i) => i,
                        Err(_e) => {
                            main_spinner.finish_with_message("Migration creation cancelled");
                            return;
                        }
                    };
                    let elem_type = match elem_type_index {
                        0 => "INTEGER",
                        1 => "TEXT",
                        2 => "VARCHAR",
                        3 => "BOOLEAN",
                        4 => "FLOAT",
                        5 => "UUID",
                        _other => "TEXT",
                    };
                    column_type = format!("{}[]", elem_type);
                }

                let nullable = match Confirm::with_theme(&theme)
                    .with_prompt("Is this column nullable?")
                    .default(false)
                    .interact()
                {
                    Ok(v) => v,
                    Err(_e) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };

                let unique = match Confirm::with_theme(&theme)
                    .with_prompt("Is this column unique?")
                    .default(false)
                    .interact()
                {
                    Ok(v) => v,
                    Err(_e) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };

                let default_value = match Input::<String>::with_theme(&theme)
                    .with_prompt("Enter default value (or leave empty for none)")
                    .allow_empty(true)
                    .interact_text()
                {
                    Ok(v) => v,
                    Err(_e) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };
                let default_value_display = if default_value.is_empty() {
                    String::new()
                } else {
                    format!("DEFAULT {} ", default_value)
                };

                let is_primary_key = if column_type == "SERIAL" {
                    true
                } else {
                    match Confirm::with_theme(&theme)
                        .with_prompt("Is this column a primary key?")
                        .default(false)
                        .interact()
                    {
                        Ok(v) => v,
                        Err(_e) => {
                            main_spinner.finish_with_message("Migration creation cancelled");
                            return;
                        }
                    }
                };

                let is_foreign_key = match Confirm::with_theme(&theme)
                    .with_prompt("Is this column a foreign key?")
                    .default(false)
                    .interact()
                {
                    Ok(v) => v,
                    Err(_e) => {
                        main_spinner.finish_with_message("Migration creation cancelled");
                        return;
                    }
                };

                if is_foreign_key {
                    let existing_tables = crate::database::seeds::get_existing_tables();
                    if existing_tables.is_empty() {
                        log_message("No existing tables found for foreign key reference.");
                    } else {
                        match Select::with_theme(&theme)
                            .with_prompt("Select referenced table")
                            .items(&existing_tables)
                            .interact()
                        {
                            Ok(index) => {
                                let referenced_table = existing_tables[index].clone();
                                let referenced_column = match Input::<String>::with_theme(&theme)
                                    .with_prompt("Enter referenced column")
                                    .default("id".to_string())
                                    .interact_text()
                                {
                                    Ok(v) => v,
                                    Err(e) => {
                                        drop(e);
                                        "id".to_string()
                                    }
                                };
                                foreign_keys.push((
                                    column_name.clone(),
                                    referenced_table.clone(),
                                    referenced_column.clone(),
                                ));
                                log_message(&format!(
                                    "Added foreign key: {} references {}({})",
                                    column_name, referenced_table, referenced_column
                                ));
                            }
                            Err(_e) => {
                                log_message("Foreign key creation cancelled.");
                            }
                        }
                    }
                }

                columns.push((
                    column_name.clone(),
                    column_type.clone(),
                    nullable,
                    unique,
                    default_value,
                    is_primary_key,
                ));

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
            1 => break,
            2 => {
                new_migration();
                return;
            }
            _other => break,
        }
    }

    step_spinner.finish_and_clear();

    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message(format!("Step {}/{}: Review migration", current_step, max_step));

    let mut up_sql_preview = if is_new_table {
        format!("CREATE TABLE IF NOT EXISTS {} (\n", table_name)
    } else {
        format!("ALTER TABLE {} ", table_name)
    };

    if is_new_table {
        for (i, (name, typ, nullable, unique, ref default, is_primary_key)) in columns.iter().enumerate() {
            up_sql_preview.push_str(&format!(
                "    {} {}{}{}{}{}",
                name,
                typ,
                if *nullable { "" } else { " NOT NULL" },
                if *unique { " UNIQUE" } else { "" },
                if default.is_empty() { String::new() } else { format!(" DEFAULT {}", default) },
                if *is_primary_key { " PRIMARY KEY" } else { "" }
            ));
            if i < columns.len() - 1 || !foreign_keys.is_empty() {
                up_sql_preview.push_str(",\n");
            } else {
                up_sql_preview.push_str("\n");
            }
        }

        for (i, (column, ref_table, ref_column)) in foreign_keys.iter().enumerate() {
            up_sql_preview.push_str(&format!(
                "    FOREIGN KEY ({}) REFERENCES {}({})",
                column, ref_table, ref_column
            ));
            if i < foreign_keys.len() - 1 {
                up_sql_preview.push_str(",\n");
            } else {
                up_sql_preview.push_str("\n");
            }
        }
        up_sql_preview.push_str(");\n");
    } else {
        for (i, (name, typ, nullable, unique, ref default, _ignored)) in columns.iter().enumerate() {
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
            up_sql_preview.push_str(&format!(
                "ADD FOREIGN KEY ({}) REFERENCES {}({})",
                column, ref_table, ref_column
            ));
            if i < foreign_keys.len() - 1 {
                up_sql_preview.push_str(", ");
            }
        }
        up_sql_preview.push_str(";\n");
    }

    let down_sql_preview = if is_new_table {
        format!("DROP TABLE {};\n", table_name)
    } else {
        "-- reverse changes here\n".to_string()
    };

    log_message("\n=== Migration Preview ===");
    log_message(&format!("Table: {}", table_name));
    log_message(&format!(
        "Type: {}",
        if is_new_table { "Create new table" } else { "Alter existing table" }
    ));
    log_message("\nUp SQL:");
    log_message(&up_sql_preview);
    log_message("\nDown SQL:");
    log_message(&down_sql_preview);
    log_message("======================\n");

    let confirm_action = match Select::with_theme(&theme)
        .with_prompt("How would you like to proceed?")
        .items(&["Create migration", "Edit migration", "🔙 Go back", "❌ Cancel"])
        .default(0)
        .interact()
    {
        Ok(index) => index,
        Err(_e) => {
            main_spinner.finish_with_message("Migration creation cancelled");
            return;
        }
    };

    match confirm_action {
        0 => {}
        1 | 2 => {
            new_migration();
            return;
        }
        _other => {
            main_spinner.finish_with_message("Migration creation cancelled");
            return;
        }
    }

    step_spinner.finish_and_clear();

    current_step += 1;
    step_spinner = multi_progress.add(ProgressBar::new_spinner());
    step_spinner.set_style(spinner_style.clone());
    step_spinner.set_message(format!("Step {}/{}: Creating migration files", current_step, max_step));

    let migration_type = if is_new_table { "create" } else { "alter" };
    let output = match Command::new("diesel")
        .args(["migration", "generate", &format!("{}_{}", migration_type, table_name)])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log_message(&format!("Failed to execute Diesel command: {}", e));
            main_spinner.finish_with_message("Migration creation failed");
            return;
        }
    };

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        log_message(&format!("Failed to generate migration: {}", error));
        main_spinner.finish_with_message("Migration creation failed");
        return;
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout_str.lines().collect();

    if lines.len() < 2 {
        log_message("Unexpected output format from Diesel command.");
        main_spinner.finish_with_message("Migration creation failed");
        return;
    }

    let up_file = lines[0].trim().replace("Creating ", "");
    let down_file = lines[1].trim().replace("Creating ", "");

    match fs::write(&up_file, &up_sql_preview) {
        Ok(()) => {}
        Err(e) => {
            log_message(&format!("Unable to write up.sql: {}", e));
            main_spinner.finish_with_message("Migration creation failed");
            return;
        }
    }
    match fs::write(&down_file, &down_sql_preview) {
        Ok(()) => {}
        Err(e) => {
            log_message(&format!("Unable to write down.sql: {}", e));
            main_spinner.finish_with_message("Migration creation failed");
            return;
        }
    }

    step_spinner.finish_and_clear();
    main_spinner.finish_with_message(format!(
        "✅ Migration for table '{}' created successfully!",
        table_name
    ));
    log_message(&format!(
        "Migration files created at:\n- {}\n- {}",
        up_file, down_file
    ));
}
