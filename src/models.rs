use crate::configs::Config;
use crate::progress::ProgressManager;
use regex::Regex;
use std::fs;
use std::io;
use std::path::Path;

fn load_schema_table_names(schema_path: &str) -> io::Result<Vec<String>> {
    let content = fs::read_to_string(schema_path)?;

    // IMPORTANT: Use a better regex that captures the actual table name correctly
    // This regex looks for table declarations like: table! { city_boundaries (id) {
    let re = Regex::new(r"table!\s*\{\s*([A-Za-z0-9_]+)\s*\(").unwrap();

    let mut tables = Vec::new();
    for cap in re.captures_iter(&content) {
        if let Some(table_name) = cap.get(1) {
            let table_name_str = table_name.as_str().to_string();
            println!("Found table in schema for models: {}", &table_name_str);
            tables.push(table_name_str);
        }
    }

    if tables.is_empty() {
        println!("WARNING: No tables found in schema file for models at {}", schema_path);
    }

    Ok(tables)
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

fn singular(table_name: &str) -> String {
    if table_name.ends_with('s') {
        table_name[..table_name.len() - 1].to_string()
    } else {
        table_name.to_string()
    }
}

fn write_model_file(config: &Config, table_name: &str, struct_name: &str) -> bool {
    let output_dir = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("models_dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("src/models/generated");

    // Create the output directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(output_dir) {
        eprintln!("Error creating directory {}: {}", output_dir, e);
        return false;
    }

    // Always use the exact table_name from schema for the file_path
    let file_path = format!("{}/{}.rs", output_dir, table_name);

    // For dsl alias, we can still use a singular form for readability
    let singular_name = singular(table_name);

    // Create model template - ensure correct exact table name is used
    let model_template = format!(
        r#"use crate::database::db::establish_connection;
use crate::database::schema::{0}::dsl::{{self as {2}_dsl}};
use crate::structs::{1};
use crate::structs::insertable::New{1};
use diesel::prelude::*;
use diesel::result::Error;
use diesel::Connection;

impl {1} {{
    pub fn get_all() -> Result<Vec<{1}>, &'static str> {{
        let mut conn = establish_connection();

        {2}_dsl::{0}
            .order({2}_dsl::id.asc())
            .load::<{1}>(&mut conn)
            .map_err(|_| "Error retrieving all {0}")
    }}

    pub fn get_by_id(id: i32) -> Result<{1}, &'static str> {{
        let mut conn = establish_connection();

        match {2}_dsl::{0}.filter({2}_dsl::id.eq(id)).first::<{1}>(&mut conn) {{
            Ok(record) => Ok(record),
            Err(_) => Err("{1} not found"),
        }}
    }}


    pub fn create(new_record: New{1}) -> Result<{1}, &'static str> {{
        let mut conn = establish_connection();
        
        conn.transaction(|conn| {{
            // Insert the new record
            let result = diesel::insert_into({2}_dsl::{0})
                .values(&new_record)
                .get_result::<{1}>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(result)
        }})
        .map_err(|e: diesel::result::Error| {{
            match e {{
                diesel::result::Error::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, _) => {{
                    "Record with these values already exists."
                }}
                _ => "Error creating new record"
            }}
        }})
    }}

    pub fn update_by_id(id: i32, updates: &New{1}) -> Result<{1}, &'static str> {{
        let mut conn = establish_connection();
        
        conn.transaction(|conn| {{
            // Get the latest record data inside the transaction
            let record = {2}_dsl::{0}
                .filter({2}_dsl::id.eq(id))
                .first::<{1}>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
            
            // Apply updates using NewStruct with AsChangeset
            let updated = diesel::update({2}_dsl::{0}.filter({2}_dsl::id.eq(id)))
                .set(updates)
                .get_result::<{1}>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(updated)
        }})
        .map_err(|e: diesel::result::Error| {{
            match e {{
                diesel::result::Error::DatabaseError(_, _) => "Database error updating record",
                _ => "Error updating record"
            }}
        }})
    }}

    pub fn delete_by_id(id: i32) -> Result<(), &'static str> {{
        let mut conn = establish_connection();

        conn.transaction(|conn| {{
            // Get the record to confirm it exists
            let _ = {2}_dsl::{0}
                .filter({2}_dsl::id.eq(id))
                .first::<{1}>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            // Delete the record
            diesel::delete({2}_dsl::{0}.filter({2}_dsl::id.eq(id)))
                .execute(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(())
        }})
        .map_err(|_: diesel::result::Error| "Error deleting record")
    }}

    pub fn count() -> Result<i64, &'static str> {{
        let mut conn = establish_connection();
        
        {2}_dsl::{0}
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(|_| "Error counting records")
    }}
}}
"#,
        table_name, struct_name, singular_name
    );

    // Write the model file
    if let Err(e) = fs::write(&file_path, model_template) {
        eprintln!("Error writing model file {}: {}", file_path, e);
        false
    } else {
        true
    }
}

fn update_mod_file(config: &Config, processed_tables: &[String]) -> bool {
    if processed_tables.is_empty() {
        return true; // Nothing to do, but not an error
    }

    let output_dir = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("models_dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("src/models/generated");

    let mod_file_path = Path::new(output_dir).join("mod.rs");
    let mut mod_file_content = fs::read_to_string(&mod_file_path).unwrap_or_default();

    let mut updated = false;
    for table_name in processed_tables {
        let mod_declaration = format!("pub mod {};", table_name);
        if !mod_file_content.contains(&mod_declaration) {
            mod_file_content.push_str(&format!("\n{}", mod_declaration));
            updated = true;
        }
        if !mod_file_content.contains(&format!("pub use {}::*;", table_name)) {
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
    progress.set_message("Generating model implementations...");

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

    // Get the ignored models list from Catalyst.toml
    // First check the proper ignore path in [codegen.models]
    let ignore_list: Vec<String> = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("models"))
        .and_then(|s| s.get("ignore"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

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

    let mut processed_tables = Vec::new();

    // Process each table
    for table_name in schema_tables {
        // Skip ignored tables - properly handle case sensitivity
        if ignore_list.iter().any(|ignored| ignored.to_lowercase() == table_name.to_lowercase()) {
            progress.set_message(&format!("Skipping ignored table: {}", table_name));
            continue;
        }

        // Use exact table_name from schema for consistency
        let struct_name = to_pascal(&table_name);

        if write_model_file(config, &table_name, &struct_name) {
            processed_tables.push(table_name);
        }
    }

    if processed_tables.is_empty() {
        progress.error("No models were generated");
        false
    } else {
        // Update mod.rs file
        if update_mod_file(config, &processed_tables) {
            progress.success(&format!("Generated {} model files", processed_tables.len()));
            true
        } else {
            progress.error("Failed to update mod.rs file");
            false
        }
    }
}
