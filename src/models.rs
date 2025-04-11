use crate::configs::Config;
use crate::progress::ProgressManager;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

// Structure to hold column information
#[derive(Debug, Clone)]
struct ColumnInfo {
    name: String,
    column_type: String,
    nullable: bool,
}

// Structure to hold relationships
#[derive(Debug, Clone)]
struct RelationshipInfo {
    source_table: String,
    source_column: String,
    target_table: String,
    target_column: String,
}

// Structure to hold table information
#[derive(Debug)]
struct TableInfo {
    name: String,
    columns: Vec<ColumnInfo>,
}

fn load_schema_table_info(schema_path: &str) -> io::Result<Vec<TableInfo>> {
    let content = fs::read_to_string(schema_path)?;

    // Extract table declarations
    let table_re = Regex::new(r"table!\s*\{\s*([A-Za-z0-9_]+)\s*\([^)]+\)\s*\{([^}]+)\}").unwrap();
    let column_re = Regex::new(r"([A-Za-z0-9_]+)\s*->\s*([^,]+)").unwrap();
    let nullable_re = Regex::new(r"Nullable<([^>]+)>").unwrap();

    let mut tables = Vec::new();

    for table_cap in table_re.captures_iter(&content) {
        let table_name = table_cap.get(1).unwrap().as_str().to_string();
        let columns_section = table_cap.get(2).unwrap().as_str();

        let mut columns = Vec::new();

        for column_cap in column_re.captures_iter(columns_section) {
            let column_name = column_cap.get(1).unwrap().as_str().to_string();
            let column_type = column_cap.get(2).unwrap().as_str().trim().to_string();

            // Check if column is nullable
            let nullable = column_type.contains("Nullable");

            // Extract the inner type if nullable
            let clean_type = if nullable {
                if let Some(inner_cap) = nullable_re.captures(&column_type) {
                    inner_cap.get(1).unwrap().as_str().trim().to_string()
                } else {
                    column_type.clone()
                }
            } else {
                column_type.clone()
            };

            columns.push(ColumnInfo {
                name: column_name,
                column_type: clean_type,
                nullable,
            });
        }

        tables.push(TableInfo { name: table_name, columns });
    }

    if tables.is_empty() {
        crate::logger::warning(&format!("No tables found in schema file for models at {}", schema_path)).unwrap_or_default();
    }

    Ok(tables)
}

fn load_schema_table_names(schema_path: &str) -> io::Result<Vec<String>> {
    let tables = load_schema_table_info(schema_path)?;
    Ok(tables.into_iter().map(|t| t.name).collect())
}

// Parse schema for relationships (joinable! macros)
fn load_schema_relationships(schema_path: &str) -> io::Result<Vec<RelationshipInfo>> {
    let content = fs::read_to_string(schema_path)?;

    // We'll use this to track which relationships we've already detected
    // to prevent duplicates from different detection methods
    let mut relationship_map: HashMap<(String, String), RelationshipInfo> = HashMap::new();

    // Match joinable! declaration
    let joinable_re = Regex::new(r"joinable!\s*\(\s*([A-Za-z0-9_]+)\s*->\s*([A-Za-z0-9_]+)\s*\(\s*([A-Za-z0-9_]+)\s*\)\s*\)").unwrap();

    for join_cap in joinable_re.captures_iter(&content) {
        let source_table = join_cap.get(1).unwrap().as_str().to_string();
        let target_table = join_cap.get(2).unwrap().as_str().to_string();
        let source_column = join_cap.get(3).unwrap().as_str().to_string();

        let key = (source_table.clone(), source_column.clone());
        relationship_map.insert(
            key,
            RelationshipInfo {
                source_table,
                source_column,
                target_table,
                target_column: "id".to_string(), // Default assumption for joinable! macros
            },
        );
    }

    // Also check for columns with _id suffix as potential foreign keys
    let tables = load_schema_table_info(schema_path)?;
    let table_map: HashMap<String, TableInfo> = tables.into_iter().map(|t| (t.name.clone(), t)).collect();

    for (table_name, table_info) in &table_map {
        for column in &table_info.columns {
            if column.name.ends_with("_id") && column.name != "id" {
                // Extract potential target table name from column
                let potential_table = column.name.trim_end_matches("_id");

                // Check if this table exists
                if table_map.contains_key(potential_table) || table_map.contains_key(&format!("{}s", potential_table)) {
                    let target_table = if table_map.contains_key(potential_table) {
                        potential_table.to_string()
                    } else {
                        format!("{}s", potential_table)
                    };

                    // Only add if not already found in joinable! macros
                    let key = (table_name.clone(), column.name.clone());
                    if !relationship_map.contains_key(&key) {
                        relationship_map.insert(
                            key,
                            RelationshipInfo {
                                source_table: table_name.clone(),
                                source_column: column.name.clone(),
                                target_table,
                                target_column: "id".to_string(),
                            },
                        );
                    }
                }
            }
        }
    }

    // Convert the HashMap values to a Vec
    let relationships: Vec<RelationshipInfo> = relationship_map.into_values().collect();

    Ok(relationships)
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

// Generate methods for boolean fields
fn generate_bool_methods(table: &TableInfo, singular_name: &str) -> String {
    let mut bool_methods = String::new();

    for column in &table.columns {
        if column.column_type == "Bool" {
            let column_name = &column.name;

            // Generate is_VALUE getter
            bool_methods.push_str(&format!(
                r#"
    pub async fn is_{0}(&self) -> bool {{
        self.{0}
    }}

    pub async fn set_{0}(&mut self, value: bool) -> Result<Self, MeltDown> {{
        let mut conn = establish_connection();
        let current_timestamp = Utc::now().timestamp();
        
        conn.transaction(|conn| {{
            let updated = diesel::update({1}_dsl::{2}.filter({1}_dsl::id.eq(self.id)))
                .set(({1}_dsl::{0}.eq(value), {1}_dsl::updated_at.eq(current_timestamp)))
                .get_result::<Self>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(updated)
        }})
        .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "set_{0}").with_context("id", self.id.to_string()))
    }}

    pub async fn set_{0}_true(&mut self) -> Result<Self, MeltDown> {{
        self.set_{0}(true).await
    }}

    pub async fn set_{0}_false(&mut self) -> Result<Self, MeltDown> {{
        self.set_{0}(false).await
    }}
"#,
                column_name, singular_name, table.name
            ));
        }
    }

    bool_methods
}

// Generate methods for timestamp fields
fn generate_timestamp_methods(table: &TableInfo, singular_name: &str) -> String {
    let mut timestamp_methods = String::new();

    let has_created_at = table.columns.iter().any(|c| c.name == "created_at" && (c.column_type == "Int8" || c.column_type == "Timestamp"));

    let has_updated_at = table.columns.iter().any(|c| c.name == "updated_at" && (c.column_type == "Int8" || c.column_type == "Timestamp"));

    if has_created_at {
        timestamp_methods.push_str(&format!(
            r#"
    pub async fn created_after(timestamp: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {0}_dsl::{1}
            .filter({0}_dsl::created_at.gt(timestamp))
            .order({0}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "created_after").with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn created_before(timestamp: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {0}_dsl::{1}
            .filter({0}_dsl::created_at.lt(timestamp))
            .order({0}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "created_before").with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn created_between(start: i64, end: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {0}_dsl::{1}
            .filter({0}_dsl::created_at.ge(start).and({0}_dsl::created_at.le(end)))
            .order({0}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "created_between").with_context("start", start.to_string()).with_context("end", end.to_string()))
    }}

    pub async fn recent(limit: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {0}_dsl::{1}
            .order({0}_dsl::created_at.desc())
            .limit(limit)
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "recent").with_context("limit", limit.to_string()))
    }}
"#,
            singular_name, table.name
        ));
    }

    if has_updated_at {
        timestamp_methods.push_str(&format!(
            r#"
    pub async fn updated_after(timestamp: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {0}_dsl::{1}
            .filter({0}_dsl::updated_at.gt(timestamp))
            .order({0}_dsl::updated_at.desc())
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "updated_after").with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn recently_updated(limit: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {0}_dsl::{1}
            .order({0}_dsl::updated_at.desc())
            .limit(limit)
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "recently_updated").with_context("limit", limit.to_string()))
    }}
"#,
            singular_name, table.name
        ));
    }

    timestamp_methods
}

// Generate methods for relationships
fn generate_relationship_methods(table_name: &str, singular_name: &str, relationships: &[RelationshipInfo]) -> String {
    let mut relationship_methods = String::new();

    // Find relationships where this table is the source
    for relationship in relationships.iter().filter(|r| r.source_table == table_name) {
        let target_table = &relationship.target_table;
        let _target_struct = to_pascal(target_table);
        let foreign_key = &relationship.source_column;

        relationship_methods.push_str(&format!(
            r#"
    pub async fn get_by_{0}({0}: i32) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {1}_dsl::{2}
            .filter({1}_dsl::{0}.eq({0}))
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_{0}").with_context("{0}", {0}.to_string()))
    }}

    pub async fn get_by_{0}_created_before({0}: i32, timestamp: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {1}_dsl::{2}
            .filter({1}_dsl::{0}.eq({0}))
            .filter({1}_dsl::created_at.lt(timestamp))
            .order({1}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_{0}_created_before").with_context("{0}", {0}.to_string()).with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn get_by_{0}_created_after({0}: i32, timestamp: i64) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection();
        
        {1}_dsl::{2}
            .filter({1}_dsl::{0}.eq({0}))
            .filter({1}_dsl::created_at.gt(timestamp))
            .order({1}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_{0}_created_after").with_context("{0}", {0}.to_string()).with_context("timestamp", timestamp.to_string()))
    }}
"#,
            foreign_key, singular_name, &table_name
        ));
    }

    relationship_methods
}

fn write_model_file(config: &Config, table: &TableInfo, relationships: &[RelationshipInfo]) -> bool {
    let output_dir = config
        .assets
        .get("codegen")
        .and_then(|codegen| codegen.get("models_dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("src/models/generated");

    // Create the output directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(output_dir) {
        crate::logger::error(&format!("Error creating directory {}: {}", output_dir, e)).unwrap_or_default();
        return false;
    }

    let table_name = &table.name;
    let struct_name = to_pascal(table_name);

    // Always use the exact table_name from schema for the file_path
    let file_path = format!("{}/{}.rs", output_dir, table_name);

    // For dsl alias, we can still use a singular form for readability
    let singular_name = singular(table_name);

    // Generate specialized methods
    let bool_methods = generate_bool_methods(table, &singular_name);
    let timestamp_methods = generate_timestamp_methods(table, &singular_name);
    let relationship_methods = generate_relationship_methods(table_name, &singular_name, relationships);

    let model_template = format!(
        r#"use crate::database::db::establish_connection;
use crate::database::schema::{0}::dsl::{{self as {2}_dsl}};
use crate::structs::{1};
use crate::structs::insertable::New{1};
use crate::meltdown::*;
use diesel::prelude::*;
use diesel::result::Error;
use diesel::Connection;
use chrono::Utc;

impl {1} {{
    pub async fn get_all() -> Result<Vec<{1}>, MeltDown> {{
        let mut conn = establish_connection();

        {2}_dsl::{0}
            .order({2}_dsl::id.asc())
            .load::<{1}>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_all"))
    }}

    pub async fn get_by_id(id: i32) -> Result<{1}, MeltDown> {{
        let mut conn = establish_connection();

        {2}_dsl::{0}
            .filter({2}_dsl::id.eq(id))
            .first::<{1}>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_id").with_context("id", id.to_string()))
    }}


    pub async fn create(new_record: New{1}) -> Result<{1}, MeltDown> {{
        let mut conn = establish_connection();
        
        conn.transaction(|conn| {{
            let result = diesel::insert_into({2}_dsl::{0})
                .values(&new_record)
                .get_result::<{1}>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(result)
        }})
        .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "create"))
    }}

    pub async fn update_by_id(id: i32, updates: &New{1}) -> Result<{1}, MeltDown> {{
        let mut conn = establish_connection();
        
        conn.transaction(|conn| {{
            let updated = diesel::update({2}_dsl::{0}.filter({2}_dsl::id.eq(id)))
                .set(updates)
                .get_result::<{1}>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(updated)
        }})
        .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "update_by_id").with_context("id", id.to_string()))
    }}

    pub async fn delete_by_id(id: i32) -> Result<(), MeltDown> {{
        let mut conn = establish_connection();

        conn.transaction(|conn| {{
            let _ = {2}_dsl::{0}
                .filter({2}_dsl::id.eq(id))
                .first::<{1}>(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            diesel::delete({2}_dsl::{0}.filter({2}_dsl::id.eq(id)))
                .execute(conn)
                .map_err(|_| Error::RollbackTransaction)?;
                
            Ok(())
        }})
        .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "delete_by_id").with_context("id", id.to_string()))
    }}

    pub async fn count() -> Result<i64, MeltDown> {{
        let mut conn = establish_connection();
        
        {2}_dsl::{0}
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "count"))
    }}{3}{4}{5}
}}
"#,
        table_name, struct_name, singular_name, bool_methods, timestamp_methods, relationship_methods
    );

    if let Err(e) = fs::write(&file_path, model_template) {
        crate::logger::error(&format!("Error writing model file {}: {}", file_path, e)).unwrap_or_default();
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
            crate::logger::error(&format!("Error writing mod.rs file: {}", e)).unwrap_or_default();
            return false;
        }
    }

    true
}

pub fn generate(config: &Config) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating enhanced model implementations...");

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

    // Load detailed schema information
    let tables = match load_schema_table_info(schema_path) {
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

    // Load relationship information
    let relationships = match load_schema_relationships(schema_path) {
        Ok(rels) => rels,
        Err(e) => {
            crate::logger::warning(&format!("Error loading relationship information: {}. Continuing without relationship methods.", e)).unwrap_or_default();
            Vec::new()
        }
    };

    let mut processed_tables = Vec::new();

    // Process each table
    for table in &tables {
        // Skip ignored tables - properly handle case sensitivity
        if ignore_list.iter().any(|ignored| ignored.to_lowercase() == table.name.to_lowercase()) {
            progress.set_message(&format!("Skipping ignored table: {}", table.name));
            continue;
        }

        if write_model_file(config, table, &relationships) {
            processed_tables.push(table.name.clone());
        }
    }

    if processed_tables.is_empty() {
        progress.error("No models were generated");
        false
    } else {
        // Update mod.rs file
        if update_mod_file(config, &processed_tables) {
            progress.success(&format!("Generated {} enhanced model files with specialized methods", processed_tables.len()));
            true
        } else {
            progress.error("Failed to update mod.rs file");
            false
        }
    }
}

