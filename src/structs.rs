use crate::configs::Config;
use crate::progress::ProgressManager;
use regex::Regex;
use std::fs;
use std::io::{self};
use std::path::Path;
use std::process::Command;

fn load_schema_table_names(schema_path: &str) -> io::Result<Vec<String>> {
    let content = fs::read_to_string(schema_path)?;
    let re = Regex::new(r"table!\s*\{\s*(\w+)\s*\(").unwrap();
    let mut tables = Vec::new();
    for cap in re.captures_iter(&content) {
        if let Some(table_name) = cap.get(1) {
            tables.push(table_name.as_str().to_string());
        }
    }
    Ok(tables)
}

fn run_diesel_ext(config: &Config) -> io::Result<String> {
    let mut command = Command::new("diesel_ext");

    if let Some(derives) = config.assets.get("codegen").and_then(|codegen| codegen.get("structs")).and_then(|s| s.get("derives")).and_then(|v| v.as_array()) {
        let derives_str = derives.iter().filter_map(|d| d.as_str()).collect::<Vec<_>>().join(", ");
        command.arg("-d").arg(derives_str);
    }

    if let Some(imports) = config.assets.get("codegen").and_then(|codegen| codegen.get("structs")).and_then(|s| s.get("imports")).and_then(|v| v.as_array()) {
        for import in imports.iter().filter_map(|imp| imp.as_str()) {
            command.arg("-I").arg(import);
        }
    }

    if let Some(schema_path) = config.assets.get("codegen").and_then(|codegen| codegen.get("schema_file")).and_then(|v| v.as_str()) {
        command.arg("-s").arg(schema_path);
    }
    command.arg("-t");

    let output = command.output()?.stdout;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn camel_to_snake(name: &str) -> String {
    let mut snake = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                snake.push('_');
            }
            snake.extend(c.to_lowercase());
        } else {
            snake.push(c);
        }
    }
    snake
}

fn to_pascal(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

fn fix_struct_name(generated_name: &str, schema_tables: &[String]) -> (String, String) {
    let candidate = camel_to_snake(generated_name);

    if schema_tables.contains(&candidate) {
        return (to_pascal(&candidate), candidate);
    }

    if !candidate.ends_with('s') {
        let candidate_plural = format!("{}s", candidate);
        if schema_tables.contains(&candidate_plural) {
            return (to_pascal(&candidate_plural), candidate_plural);
        }
    }

    (generated_name.to_string(), candidate)
}

fn extract_struct_name(struct_def: &str) -> Option<&str> {
    struct_def.lines().find(|line| line.trim().starts_with("pub struct")).and_then(|line| line.split_whitespace().nth(2))
}

fn parse_and_process_structs(content: &str, config: &Config, schema_tables: &[String]) -> Option<Vec<String>> {
    // Single progress tracker for the entire operation
    let progress = ProgressManager::new_spinner();
    progress.set_message("Processing struct definitions...");

    let output_dir = config.assets.get("codegen").and_then(|codegen| codegen.get("structs_dir")).and_then(|v| v.as_str()).unwrap_or("src/structs");

    let ignore_list: Vec<String> = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("structs"))
        .and_then(|s| s.get("ignored_structs"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut current_struct = String::new();
    let mut inside_struct = false;

    let mut processed_tables = Vec::new();

    for line in content.lines() {
        if line.trim().starts_with("#[derive") {
            inside_struct = true;
        }
        if inside_struct {
            current_struct.push_str(line);
            current_struct.push('\n');
            if line.trim().ends_with('}') {
                if let Some(generated_name) = extract_struct_name(&current_struct) {
                    let (_fixed_name, table_name) = fix_struct_name(generated_name, schema_tables);

                    if ignore_list.iter().any(|ignored| ignored.eq_ignore_ascii_case(&table_name)) {
                        current_struct.clear();
                        inside_struct = false;
                        continue;
                    }
                    let (fixed_name, table_name) = fix_struct_name(generated_name, schema_tables);
                    if write_struct_file(config, &fixed_name, &table_name, &current_struct, output_dir) {
                        processed_tables.push(table_name);
                    }
                }
                current_struct.clear();
                inside_struct = false;
            }
        }
    }

    if processed_tables.is_empty() {
        progress.error("No structs were processed");
        None
    } else {
        // Update mod.rs file
        if update_mod_file(config, &processed_tables) {
            // Show a single consolidated message
            progress.success(&format!("Generated {} struct files from schema", processed_tables.len()));
            Some(processed_tables)
        } else {
            progress.error("Failed to update mod.rs file");
            None
        }
    }
}

fn check_migration_for_serial_fields(table_name: &str) -> Vec<String> {
    // Find migration files for this table
    let migrations_dir = "src/database/migrations";
    let mut serial_fields = Vec::new();
    let mut auto_fields = Vec::new();

    // Try to find migration files
    if let Ok(entries) = fs::read_dir(migrations_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    // Check if this migration directory has an up.sql file
                    let up_sql_path = entry.path().join("up.sql");
                    if up_sql_path.exists() {
                        // Read the up.sql file
                        if let Ok(sql_content) = fs::read_to_string(&up_sql_path) {
                            // Look for CREATE TABLE statements for this table
                            let table_pattern = format!("CREATE TABLE {}[\\s\\n]*\\(", table_name);
                            let table_re = Regex::new(&table_pattern).unwrap_or(Regex::new("this will never match").unwrap());

                            if table_re.is_match(&sql_content) {
                                // Found the migration file for this table
                                // Extract SERIAL fields
                                let lines: Vec<&str> = sql_content.lines().collect();
                                for line in lines {
                                    let trimmed = line.trim();

                                    // Look for SERIAL keyword and extract field name
                                    if trimmed.contains("SERIAL") {
                                        if let Some(field_name) = trimmed.split_whitespace().next() {
                                            serial_fields.push(field_name.to_string());
                                        }
                                    }

                                    // Look for DEFAULT expressions
                                    if trimmed.contains("DEFAULT") {
                                        if let Some(field_name) = trimmed.split_whitespace().next() {
                                            auto_fields.push(field_name.to_string());
                                        }
                                    }
                                }

                                // No need to check other migration files once we found the right one
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Add common auto-generated fields
    auto_fields.extend(vec!["id".to_string(), "created_at".to_string(), "updated_at".to_string()]);

    // Combine both lists
    let mut result = Vec::new();
    result.extend(serial_fields);
    result.extend(auto_fields);
    result
}

fn write_struct_file(config: &Config, fixed_struct_name: &str, table_name: &str, struct_def: &str, output_dir: &str) -> bool {
    // Create the output directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(output_dir) {
        eprintln!("Error creating directory {}: {}", output_dir, e);
        return false;
    }

    // Create insertable directory for New* structs
    let insertable_dir = format!("{}/insertable", output_dir);
    if let Err(_e) = fs::create_dir_all(&insertable_dir) {
        eprintln!("Error creating insertable directory");
        return false;
    }

    // Get the auto-generated fields for this table by examining migration files
    let auto_fields = check_migration_for_serial_fields(table_name);

    // Process the main struct definition
    let mut new_struct_def = struct_def
        .lines()
        .map(|line| {
            if line.trim().starts_with("pub struct") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 2 {
                    return line.replacen(parts[2], fixed_struct_name, 1);
                }
            }
            line.to_string()
        })
        .collect::<Vec<String>>()
        .join("\n");

    // Extract field definitions for insertable structs only
    let re = Regex::new(r"(?s)pub struct.*?\{(.*?)\}").unwrap(); // (?s) enables dot-all mode for regex
    let mut insertable_fields = String::new();

    if let Some(captures) = re.captures(&new_struct_def) {
        if let Some(fields_match) = captures.get(1) {
            let fields = fields_match.as_str();

            // Process each field
            for line in fields.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("pub ") {
                    // Split the field into name and type
                    let field_parts: Vec<&str> = trimmed.split(':').collect();
                    if field_parts.len() > 1 {
                        let field_name = field_parts[0].trim();
                        let field_type = field_parts[1].trim().trim_end_matches(',');

                        // For insertable: skip auto-generated fields (SERIAL, DEFAULT, etc.)
                        // Check if this field is in our auto-generated fields list
                        let is_auto_field = auto_fields.iter().any(|af| field_name.ends_with(&format!(" {}", af)) || field_name.ends_with(&format!(":{}", af)));

                        if !trimmed.contains("primary_key") && !is_auto_field {
                            insertable_fields.push_str(&format!("    {}: {},\n", field_name, field_type));
                        }
                    }
                }
            }
        }
    }

    // No ChangeSet structs - they're removed

    // Create the insertable struct definition (will go in a separate file)
    let insertable_struct = format!(
        r#"use crate::database::schema::{0};
use diesel::{{Insertable, Queryable, AsChangeset}};
use serde::{{Serialize, Deserialize}};

#[derive(Debug, Insertable, AsChangeset, Serialize, Deserialize)]
#[diesel(table_name = {0})]
pub struct New{1} {{
{2}}}
"#,
        table_name, fixed_struct_name, insertable_fields
    );

    // Add the imports
    let additional_imports: Vec<String> = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("structs"))
        .and_then(|s| s.get("imports"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let mut imports = vec!["diesel::Insertable".to_string(), "diesel::AsChangeset".to_string()];
    imports.extend(additional_imports);

    let additional_imports_str: String = imports.iter().map(|imp| format!("use {};", imp)).collect::<Vec<String>>().join("\n") + "\n";

    // Combine everything for the main struct file - no ChangeSet structs anymore
    if !new_struct_def.contains(&format!("use crate::database::schema::{}", table_name)) {
        new_struct_def = format!("use crate::database::schema::{};\n{}{}", table_name, additional_imports_str, new_struct_def);
    } else {
        new_struct_def = format!("{}\n{}", additional_imports_str, new_struct_def);
    }

    // Write the main struct file
    let file_name = format!("{}/{}.rs", output_dir, table_name);
    let insertable_file_name = format!("{}/{}.rs", insertable_dir, table_name);

    let struct_write_ok = if let Err(e) = fs::write(&file_name, new_struct_def) {
        eprintln!("Error writing struct file {}: {}", file_name, e);
        false
    } else {
        true
    };

    // Write the insertable struct file
    let insertable_write_ok = if let Err(e) = fs::write(&insertable_file_name, insertable_struct) {
        eprintln!("Error writing insertable struct file {}: {}", insertable_file_name, e);
        false
    } else {
        true
    };

    // Update the insertable mod.rs file to include the new file
    let insertable_mod_path = format!("{}/insertable/mod.rs", output_dir);
    let mut mod_content = fs::read_to_string(&insertable_mod_path).unwrap_or_default();
    let mod_declaration = format!("pub mod {};", table_name);
    let pub_use = format!("pub use {}::*;", table_name);

    if !mod_content.contains(&mod_declaration) {
        mod_content.push_str(&format!("\n{}", mod_declaration));
        mod_content.push_str(&format!("\n{}", pub_use));

        if let Err(e) = fs::write(&insertable_mod_path, mod_content) {
            eprintln!("Error updating insertable mod.rs: {}", e);
        }
    }

    struct_write_ok && insertable_write_ok
}

fn update_mod_file(config: &Config, struct_table_names: &[String]) -> bool {
    if struct_table_names.is_empty() {
        return true; // Nothing to do, but not an error
    }

    let output_dir = config.assets.get("codegen").and_then(|codegen| codegen.get("structs_dir")).and_then(|v| v.as_str()).unwrap_or("src/structs");

    let mod_file_path = Path::new(output_dir).join("mod.rs");
    let mut mod_file_content = fs::read_to_string(&mod_file_path).unwrap_or_default();

    let mut updated = false;
    for table_name in struct_table_names {
        let mod_declaration = format!("pub mod {};", table_name);
        if !mod_file_content.contains(&mod_declaration) {
            mod_file_content.push_str(&format!("\n{}", mod_declaration));
            updated = true;
        }
        if !mod_file_content.contains(&format!("pub use {}::{};", table_name, to_pascal(table_name))) && !mod_file_content.contains(&format!("pub use {}::*;", table_name)) {
            mod_file_content.push_str(&format!("\npub use {}::*;", table_name));
            updated = true;
        }
    }

    if updated {
        if let Err(e) = fs::write(&mod_file_path, mod_file_content) {
            eprintln!("Error writing mod.rs file: {}", e);
            return false;
        }
    }

    true
}

pub fn generate(config: &Config) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating database structs...");

    // Get schema file path
    let schema_path = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("schema_file"))
        .and_then(|v| v.as_str())
        .unwrap_or("src/database/schema.rs");

    // Check if schema file exists
    if !Path::new(schema_path).exists() {
        progress.error(&format!("Schema file not found at {}", schema_path));
        return false;
    }

    // Get ignored structs list from Catalyst.toml - proper path is codegen.structs.ignored_structs
    let ignore_list: Vec<String> = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("structs"))
        .and_then(|s| s.get("ignored_structs"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Print ignored structs for debugging
    if !ignore_list.is_empty() {
        progress.set_message(&format!("Ignoring struct generation for: {}", ignore_list.join(", ")));
    }

    // Load schema table names
    let schema_tables = match load_schema_table_names(schema_path) {
        Ok(tables) => {
            if tables.is_empty() {
                progress.error("No tables found in schema file");
                return false;
            }
            tables
        }
        Err(e) => {
            progress.error(&format!("Error loading schema file: {}", e));
            return false;
        }
    };

    // Create output directory
    let output_dir = config.assets.get("codegen").and_then(|codegen| codegen.get("structs_dir")).and_then(|v| v.as_str()).unwrap_or("src/structs");

    if let Err(e) = fs::create_dir_all(output_dir) {
        progress.error(&format!("Error creating structs directory: {}", e));
        return false;
    }

    // Also create the insertable directory
    let insertable_dir = format!("{}/insertable", output_dir);
    if let Err(_) = fs::create_dir_all(&insertable_dir) {
        progress.error("Error creating insertable directory");
        return false;
    }

    // Initialize the insertable/mod.rs file if it doesn't exist
    let insertable_mod_path = format!("{}/insertable/mod.rs", output_dir);
    if !Path::new(&insertable_mod_path).exists() {
        if let Err(_) = fs::write(&insertable_mod_path, "// Auto-generated insertable struct exports\n") {
            progress.error("Error creating insertable/mod.rs file");
            return false;
        }
    }

    // Run diesel_ext
    let output = match run_diesel_ext(config) {
        Ok(output) => {
            if output.trim().is_empty() {
                progress.error("diesel_ext command produced no output");
                return false;
            }
            output
        }
        Err(e) => {
            progress.error(&format!("Error running diesel_ext: {}", e));
            return false;
        }
    };

    // Parse and process
    if let Some(_tables) = parse_and_process_structs(&output, config, &schema_tables) {
        // Success message already shown in parse_and_process_structs
        true
    } else {
        // Error message already shown in parse_and_process_structs
        false
    }
}
