use crate::codegen::header;
use crate::configs::Config;
use crate::error::BlastResult;
use crate::progress::ProgressManager;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::process::Command;

fn load_schema_table_names(schema_path: &str) -> BlastResult<Vec<String>> {
    let content = fs::read_to_string(schema_path)?;

    let re = Regex::new(r"table!\s*\{\s*([A-Za-z0-9_]+)\s*\(")?;

    let mut tables = Vec::new();
    for cap in re.captures_iter(&content) {
        match cap.get(1) {
            None => continue,
            Some(table_name) => {
                tables.push(table_name.as_str().to_string());
            }
        }
    }

    if tables.is_empty() {
        drop(crate::logger::warning(&format!("No tables found in schema file at {}", schema_path)));
    }

    Ok(tables)
}

fn run_diesel_ext(_config: &Config) -> BlastResult<String> {
    let mut command = Command::new("diesel_ext");
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
            let Some(f) = chars.next() else {
                return String::new();
            };
            f.to_uppercase().collect::<String>() + chars.as_str()
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

    for table_name in schema_tables {
        if table_name.to_lowercase().contains(&candidate.to_lowercase()) || candidate.to_lowercase().contains(&table_name.to_lowercase()) {
            return (to_pascal(table_name), table_name.clone());
        }
    }

    (generated_name.to_string(), candidate)
}

fn extract_struct_name(struct_def: &str) -> Option<&str> {
    struct_def.lines().find(|line| line.trim().starts_with("pub struct")).and_then(|line| line.split_whitespace().nth(2))
}

fn parse_and_process_structs(content: &str, config: &Config, schema_tables: &[String], marker: &str) -> Option<Vec<String>> {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Processing struct definitions...");

    let output_dir = "src/structs";

    let ignore_list: Vec<String> = Vec::new();

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
                let Some(generated_name) = extract_struct_name(&current_struct) else {
                    current_struct.clear();
                    inside_struct = false;
                    continue;
                };
                let (_fixed_name, table_name) = fix_struct_name(generated_name, schema_tables);

                if ignore_list.iter().any(|ignored| ignored.eq_ignore_ascii_case(&table_name)) {
                    current_struct.clear();
                    inside_struct = false;
                    continue;
                }
                let (fixed_name, table_name) = fix_struct_name(generated_name, schema_tables);

                if write_struct_file(config, &fixed_name, &table_name, &current_struct, output_dir, marker) {
                    processed_tables.push(table_name);
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
        if update_mod_file(config, &processed_tables) {
            progress.success(&format!("Generated {} struct files from schema", processed_tables.len()));
            Some(processed_tables)
        } else {
            progress.error("Failed to update mod.rs file");
            None
        }
    }
}

fn check_migration_for_serial_fields(table_name: &str) -> Vec<String> {
    let migrations_dir = "src/database/migrations";
    let mut serial_fields = Vec::new();
    let mut auto_fields = Vec::new();

    let entries = match fs::read_dir(migrations_dir) {
        Err(_e) => {
            auto_fields.extend(vec!["id".to_string(), "created_at".to_string(), "updated_at".to_string()]);
            return auto_fields;
        }
        Ok(entries) => entries,
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Err(_e) => continue,
            Ok(ft) => ft,
        };
        if !file_type.is_dir() {
            continue;
        }
        let up_sql_path = entry.path().join("up.sql");
        if !up_sql_path.exists() {
            continue;
        }
        let sql_content = match fs::read_to_string(&up_sql_path) {
            Err(_e) => continue,
            Ok(c) => c,
        };
        let table_pattern = format!("CREATE TABLE {}[\\s\\n]*\\(", table_name);
        let table_re = match Regex::new(&table_pattern) {
            Err(_e) => continue,
            Ok(re) => re,
        };

        if !table_re.is_match(&sql_content) {
            continue;
        }

        for line in sql_content.lines() {
            let trimmed = line.trim();

            if trimmed.contains("SERIAL") {
                let Some(field_name) = trimmed.split_whitespace().next() else {
                    continue;
                };
                serial_fields.push(field_name.to_string());
            }

            if trimmed.contains("DEFAULT") {
                let Some(field_name) = trimmed.split_whitespace().next() else {
                    continue;
                };
                auto_fields.push(field_name.to_string());
            }
        }

        break;
    }

    auto_fields.extend(vec!["id".to_string(), "created_at".to_string(), "updated_at".to_string()]);

    let mut result = Vec::new();
    result.extend(serial_fields);
    result.extend(auto_fields);
    result
}

fn write_struct_file(_config: &Config, fixed_struct_name: &str, table_name: &str, struct_def: &str, output_dir: &str, marker: &str) -> bool {
    if let Err(e) = fs::create_dir_all(output_dir) {
        drop(crate::logger::error(&format!("Error creating directory {}: {}", output_dir, e)));
        return false;
    }

    let insertable_dir = format!("{}/insertable", output_dir);
    if let Err(e) = fs::create_dir_all(&insertable_dir) {
        drop(crate::logger::error(&format!("Error creating insertable directory: {}", e)));
        return false;
    }

    let insertable_ignore_list: Vec<String> = Vec::new();

    let skip_insertable = insertable_ignore_list.iter().any(|ignored: &String| ignored.eq_ignore_ascii_case(table_name));

    let auto_fields = check_migration_for_serial_fields(table_name);

    let new_struct_def = struct_def
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

    let re = match Regex::new(r"(?s)pub struct.*?\{(.*?)\}") {
        Err(e) => {
            drop(crate::logger::error(&format!("Error compiling struct regex: {}", e)));
            return false;
        }
        Ok(re) => re,
    };
    let mut insertable_fields = String::new();

    let captures = match re.captures(&new_struct_def) {
        None => {
            drop(crate::logger::error(&format!("No struct body found in definition for {}", fixed_struct_name)));
            return false;
        }
        Some(c) => c,
    };
    let fields_match = match captures.get(1) {
        None => {
            drop(crate::logger::error(&format!("No capture group 1 in struct body for {}", fixed_struct_name)));
            return false;
        }
        Some(m) => m,
    };
    let fields = fields_match.as_str();

    for line in fields.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub ") {
            let field_parts: Vec<&str> = trimmed.split(':').collect();
            if field_parts.len() > 1 {
                let field_name = field_parts[0].trim();
                let field_type = field_parts[1].trim().trim_end_matches(',');

                let is_auto_field = auto_fields.iter().any(|af| field_name.ends_with(&format!(" {}", af)) || field_name.ends_with(&format!(":{}", af)));

                if !trimmed.contains("primary_key") && !is_auto_field {
                    insertable_fields.push_str(&format!("    {}: {},\n", field_name, field_type));
                }
            }
        }
    }

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

    let global_imports: Vec<String> = Vec::new();
    let struct_specific_imports: Vec<String> = Vec::new();

    let mut imports = vec!["diesel::Insertable".to_string(), "diesel::AsChangeset".to_string()];
    imports.extend(global_imports);
    imports.extend(struct_specific_imports);

    let additional_imports_str: String = imports.iter().map(|imp| format!("use {};", imp)).collect::<Vec<String>>().join("\n") + "\n";

    let schema_import_pattern = match Regex::new(r"use crate::database::schema::[^;]+;") {
        Err(e) => {
            drop(crate::logger::error(&format!("Error compiling schema import regex: {}", e)));
            return false;
        }
        Ok(re) => re,
    };
    let mut final_struct_def = schema_import_pattern.replace_all(&new_struct_def, "").to_string();

    drop(crate::logger::debug(&format!("For struct: {}, using table_name: {} for schema import", fixed_struct_name, table_name)));

    final_struct_def = format!(
        "{}use crate::database::schema::{};\n{}{}",
        marker,
        table_name,
        additional_imports_str,
        final_struct_def
    );

    let file_name = format!("{}/{}.rs", output_dir, table_name);

    drop(crate::logger::debug(&format!("Writing struct file: {} for table: {}", file_name, table_name)));

    let struct_write_ok = if let Err(e) = fs::write(&file_name, final_struct_def) {
        drop(crate::logger::error(&format!("Error writing struct file {}: {}", file_name, e)));
        false
    } else {
        true
    };

    let insertable_write_ok = if skip_insertable {
        true
    } else {
        let insertable_file_name = format!("{}/{}.rs", insertable_dir, table_name);

        drop(crate::logger::debug(&format!("Writing insertable struct file: {} for table: {}", insertable_file_name, table_name)));

        let insertable_with_marker = format!("{}{}", marker, insertable_struct);
        if let Err(e) = fs::write(&insertable_file_name, insertable_with_marker) {
            drop(crate::logger::error(&format!("Error writing insertable struct file {}: {}", insertable_file_name, e)));
            false
        } else {
            let insertable_mod_path = format!("{}/insertable/mod.rs", output_dir);
            let mut mod_content = if Path::new(&insertable_mod_path).exists() {
                match fs::read_to_string(&insertable_mod_path) {
                    Err(e) => {
                        drop(crate::logger::error(&format!("Error reading insertable mod.rs: {}", e)));
                        return false;
                    }
                    Ok(c) => c,
                }
            } else {
                String::new()
            };
            let mod_declaration = format!("pub mod {};", table_name);
            let pub_use = format!("pub use {}::*;", table_name);

            if !mod_content.contains(&mod_declaration) {
                mod_content.push_str(&format!("\n{}", mod_declaration));
                mod_content.push_str(&format!("\n{}", pub_use));

                if let Err(e) = fs::write(&insertable_mod_path, mod_content) {
                    drop(crate::logger::error(&format!("Error updating insertable mod.rs: {}", e)));
                    false
                } else {
                    true
                }
            } else {
                true
            }
        }
    };

    struct_write_ok && insertable_write_ok
}

fn update_mod_file(_config: &Config, struct_table_names: &[String]) -> bool {
    if struct_table_names.is_empty() {
        return true;
    }

    let output_dir = "src/structs";

    let mod_file_path = Path::new(output_dir).join("mod.rs");
    let mut mod_file_content = if mod_file_path.exists() {
        match fs::read_to_string(&mod_file_path) {
            Err(e) => {
                drop(crate::logger::error(&format!("Error reading mod.rs file: {}", e)));
                return false;
            }
            Ok(c) => c,
        }
    } else {
        String::new()
    };

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
            drop(crate::logger::error(&format!("Error writing mod.rs file: {}", e)));
            return false;
        }
    }

    true
}

pub fn generate(config: &Config) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating database structs...");

    let schema_path = "src/database/schema.rs";

    if !Path::new(schema_path).exists() {
        progress.error(&format!("Schema file not found at {}", schema_path));
        return false;
    }

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

    let output_dir = "src/structs";

    if let Err(e) = fs::create_dir_all(output_dir) {
        progress.error(&format!("Error creating structs directory: {}", e));
        return false;
    }

    let insertable_dir = format!("{}/insertable", output_dir);
    if let Err(e) = fs::create_dir_all(&insertable_dir) {
        progress.error(&format!("Error creating insertable directory: {}", e));
        return false;
    }

    let insertable_mod_path = format!("{}/insertable/mod.rs", output_dir);
    if !Path::new(&insertable_mod_path).exists() {
        if let Err(e) = fs::write(&insertable_mod_path, "// Auto-generated insertable struct exports\n") {
            progress.error(&format!("Error creating insertable/mod.rs file: {}", e));
            return false;
        }
    }

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

    let marker = match header::marker_for_schema(Path::new(".")) {
        Ok(m) => m,
        Err(e) => {
            progress.error(&format!("Error computing schema hash marker: {}", e));
            return false;
        }
    };

    parse_and_process_structs(&output, config, &schema_tables, &marker).is_some()
}
