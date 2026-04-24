use crate::configs::Config;
use crate::error::BlastResult;
use crate::progress::ProgressManager;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
struct ColumnInfo {
    name: String,
    column_type: String,
}

#[derive(Debug, Clone)]
struct RelationshipInfo {
    source_table: String,
    source_column: String,
    target_table: String,
}

#[derive(Debug)]
struct TableInfo {
    name: String,
    columns: Vec<ColumnInfo>,
}

fn load_schema_table_info(schema_path: &str) -> BlastResult<Vec<TableInfo>> {
    let content = fs::read_to_string(schema_path)?;

    let table_re = Regex::new(r"table!\s*\{\s*([A-Za-z0-9_]+)\s*\([^)]+\)\s*\{([^}]+)\}")?;
    let column_re = Regex::new(r"([A-Za-z0-9_]+)\s*->\s*([^,]+)")?;
    let nullable_re = Regex::new(r"Nullable<([^>]+)>")?;

    let mut tables = Vec::new();

    for table_cap in table_re.captures_iter(&content) {
        let table_name = table_cap
            .get(1)
            .ok_or_else(|| crate::error::BlastError::Invalid("table regex group 1 missing".into()))?
            .as_str()
            .to_string();
        let columns_section = table_cap
            .get(2)
            .ok_or_else(|| crate::error::BlastError::Invalid("table regex group 2 missing".into()))?
            .as_str();

        let mut columns = Vec::new();

        for column_cap in column_re.captures_iter(columns_section) {
            let column_name = column_cap
                .get(1)
                .ok_or_else(|| crate::error::BlastError::Invalid("column regex group 1 missing".into()))?
                .as_str()
                .to_string();
            let column_type = column_cap
                .get(2)
                .ok_or_else(|| crate::error::BlastError::Invalid("column regex group 2 missing".into()))?
                .as_str()
                .trim()
                .to_string();

            let is_nullable = column_type.contains("Nullable");

            let clean_type = if is_nullable {
                let Some(inner_cap) = nullable_re.captures(&column_type) else {
                    columns.push(ColumnInfo { name: column_name, column_type });
                    continue;
                };
                inner_cap
                    .get(1)
                    .ok_or_else(|| crate::error::BlastError::Invalid("nullable regex group 1 missing".into()))?
                    .as_str()
                    .trim()
                    .to_string()
            } else {
                column_type.clone()
            };

            columns.push(ColumnInfo {
                name: column_name,
                column_type: clean_type,
            });
        }

        tables.push(TableInfo { name: table_name, columns });
    }

    if tables.is_empty() {
        if let Err(e) = crate::logger::warning(&format!("No tables found in schema file for models at {}", schema_path)) {
            eprintln!("logger warning failed: {}", e);
        }
    }

    Ok(tables)
}

fn load_schema_relationships(schema_path: &str) -> BlastResult<Vec<RelationshipInfo>> {
    let content = fs::read_to_string(schema_path)?;

    let mut relationship_map: HashMap<(String, String), RelationshipInfo> = HashMap::new();

    let joinable_re = Regex::new(r"joinable!\s*\(\s*([A-Za-z0-9_]+)\s*->\s*([A-Za-z0-9_]+)\s*\(\s*([A-Za-z0-9_]+)\s*\)\s*\)")?;

    for join_cap in joinable_re.captures_iter(&content) {
        let source_table = join_cap
            .get(1)
            .ok_or_else(|| crate::error::BlastError::Invalid("joinable regex group 1 missing".into()))?
            .as_str()
            .to_string();
        let target_table = join_cap
            .get(2)
            .ok_or_else(|| crate::error::BlastError::Invalid("joinable regex group 2 missing".into()))?
            .as_str()
            .to_string();
        let source_column = join_cap
            .get(3)
            .ok_or_else(|| crate::error::BlastError::Invalid("joinable regex group 3 missing".into()))?
            .as_str()
            .to_string();

        let key = (source_table.clone(), source_column.clone());
        relationship_map.insert(
            key,
            RelationshipInfo {
                source_table,
                source_column,
                target_table,
            },
        );
    }

    let tables = load_schema_table_info(schema_path)?;
    let table_map: HashMap<String, TableInfo> = tables.into_iter().map(|t| (t.name.clone(), t)).collect();

    for (table_name, table_info) in &table_map {
        for column in &table_info.columns {
            if column.name.ends_with("_id") && column.name != "id" {
                let potential_table = column.name.trim_end_matches("_id");

                if table_map.contains_key(potential_table) || table_map.contains_key(&format!("{}s", potential_table)) {
                    let target_table = if table_map.contains_key(potential_table) {
                        potential_table.to_string()
                    } else {
                        format!("{}s", potential_table)
                    };

                    let key = (table_name.clone(), column.name.clone());
                    if !relationship_map.contains_key(&key) {
                        relationship_map.insert(
                            key,
                            RelationshipInfo {
                                source_table: table_name.clone(),
                                source_column: column.name.clone(),
                                target_table,
                            },
                        );
                    }
                }
            }
        }
    }

    let relationships: Vec<RelationshipInfo> = relationship_map.into_values().collect();

    Ok(relationships)
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

fn singular(table_name: &str) -> String {
    if table_name.ends_with('s') {
        table_name[..table_name.len() - 1].to_string()
    } else {
        table_name.to_string()
    }
}

fn generate_bool_methods(table: &TableInfo, singular_name: &str) -> String {
    let mut bool_methods = String::new();

    for column in &table.columns {
        if column.column_type == "Bool" {
            let column_name = &column.name;

            bool_methods.push_str(&format!(
                r#"
    pub async fn is_{0}(&self, tenant_name: &str) -> bool {{
        self.{0}
    }}

    pub async fn set_{0}(&mut self, value: bool, tenant_name: &str) -> Result<Self, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;
        let current_timestamp = Utc::now().timestamp();
        let item_id = self.id;

        let updated = diesel::update({1}_dsl::{2}.filter({1}_dsl::id.eq(item_id)))
            .set(({1}_dsl::{0}.eq(value), {1}_dsl::updated_at.eq(current_timestamp)))
            .get_result::<Self>(&mut conn)
            .await
            .map_err(|e| MeltDown::from(e).with_context("operation", "set_{0}").with_context("id", item_id.to_string()))?;

        *self = updated.clone();
        Ok(updated)
    }}

    pub async fn set_{0}_true(&mut self, tenant_name: &str) -> Result<Self, MeltDown> {{
        self.set_{0}(true, tenant_name).await
    }}

    pub async fn set_{0}_false(&mut self, tenant_name: &str) -> Result<Self, MeltDown> {{
        self.set_{0}(false, tenant_name).await
    }}
"#,
                column_name, singular_name, table.name
            ));
        }
    }

    bool_methods
}

fn generate_timestamp_methods(table: &TableInfo, singular_name: &str) -> String {
    let mut timestamp_methods = String::new();

    let created_at_column = table.columns.iter().find(|c| c.name == "created_at");
    let updated_at_column = table.columns.iter().find(|c| c.name == "updated_at");

    let has_created_at = matches!(created_at_column, Some(c) if c.column_type == "Int8" || c.column_type == "Timestamp");
    let has_updated_at = matches!(updated_at_column, Some(c) if c.column_type == "Int8" || c.column_type == "Timestamp");
    let has_int8_created_at = matches!(created_at_column, Some(c) if c.column_type == "Int8");
    let has_int8_updated_at = matches!(updated_at_column, Some(c) if c.column_type == "Int8");

    if has_created_at && has_int8_created_at {
        timestamp_methods.push_str(&format!(
            r#"
    pub async fn created_after(timestamp: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {0}_dsl::{1}
            .filter({0}_dsl::created_at.gt(timestamp))
            .order({0}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "created_after").with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn created_before(timestamp: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {0}_dsl::{1}
            .filter({0}_dsl::created_at.lt(timestamp))
            .order({0}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "created_before").with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn created_between(start: i64, end: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {0}_dsl::{1}
            .filter({0}_dsl::created_at.ge(start).and({0}_dsl::created_at.le(end)))
            .order({0}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "created_between").with_context("start", start.to_string()).with_context("end", end.to_string()))
    }}

    pub async fn recent(limit: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {0}_dsl::{1}
            .order({0}_dsl::created_at.desc())
            .limit(limit)
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "recent").with_context("limit", limit.to_string()))
    }}
"#,
            singular_name, table.name
        ));
    }

    if has_updated_at && has_int8_updated_at {
        timestamp_methods.push_str(&format!(
            r#"
    pub async fn updated_after(timestamp: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {0}_dsl::{1}
            .filter({0}_dsl::updated_at.gt(timestamp))
            .order({0}_dsl::updated_at.desc())
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "updated_after").with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn recently_updated(limit: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {0}_dsl::{1}
            .order({0}_dsl::updated_at.desc())
            .limit(limit)
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "recently_updated").with_context("limit", limit.to_string()))
    }}
"#,
            singular_name, table.name
        ));
    }

    timestamp_methods
}

fn generate_relationship_methods(table: &TableInfo, table_name: &str, singular_name: &str, relationships: &[RelationshipInfo]) -> String {
    let mut relationship_methods = String::new();

    let created_at_column = table.columns.iter().find(|c| c.name == "created_at");
    let has_int8_created_at = matches!(created_at_column, Some(c) if c.column_type == "Int8");

    for relationship in relationships.iter().filter(|r| r.source_table == table_name) {
        let target_table = &relationship.target_table;
        let _target_struct = to_pascal(target_table);
        let foreign_key = &relationship.source_column;

        relationship_methods.push_str(&format!(
            r#"
    pub async fn get_by_{0}({0}: i32, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {1}_dsl::{2}
            .filter({1}_dsl::{0}.eq({0}))
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_{0}").with_context("{0}", {0}.to_string()))
    }}
"#,
            foreign_key, singular_name, &table_name
        ));

        if has_int8_created_at {
            relationship_methods.push_str(&format!(
                r#"
    pub async fn get_by_{0}_created_before({0}: i32, timestamp: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {1}_dsl::{2}
            .filter({1}_dsl::{0}.eq({0}))
            .filter({1}_dsl::created_at.lt(timestamp))
            .order({1}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_{0}_created_before").with_context("{0}", {0}.to_string()).with_context("timestamp", timestamp.to_string()))
    }}

    pub async fn get_by_{0}_created_after({0}: i32, timestamp: i64, tenant_name: &str) -> Result<Vec<Self>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {1}_dsl::{2}
            .filter({1}_dsl::{0}.eq({0}))
            .filter({1}_dsl::created_at.gt(timestamp))
            .order({1}_dsl::created_at.desc())
            .load::<Self>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_{0}_created_after").with_context("{0}", {0}.to_string()).with_context("timestamp", timestamp.to_string()))
    }}
"#,
                foreign_key, singular_name, &table_name
            ));
        }
    }

    relationship_methods
}

fn write_model_file(_config: &Config, table: &TableInfo, relationships: &[RelationshipInfo]) -> bool {
    let output_dir = "src/models/generated";

    if let Err(e) = fs::create_dir_all(output_dir) {
        if let Err(log_err) = crate::logger::error(&format!("Error creating directory {}: {}", output_dir, e)) {
            eprintln!("logger error failed: {}", log_err);
        }
        return false;
    }

    let table_name = &table.name;
    let struct_name = to_pascal(table_name);

    let file_path = format!("{}/{}.rs", output_dir, table_name);

    let singular_name = singular(table_name);

    let bool_methods = generate_bool_methods(table, &singular_name);
    let timestamp_methods = generate_timestamp_methods(table, &singular_name);
    let relationship_methods = generate_relationship_methods(table, table_name, &singular_name, relationships);

    let model_template = format!(
        r#"use crate::database::db::{{establish_connection_with_tenant}};
use crate::database::schema::{0}::dsl::{{self as {2}_dsl}};
use crate::structs::*;
use crate::meltdown::*;
use diesel::prelude::*;
use diesel_async::{{AsyncConnection, RunQueryDsl}};
use diesel_async::scoped_futures::ScopedFutureExt;
use chrono::Utc;

impl {1} {{
    pub async fn get_all(tenant_name: &str) -> Result<Vec<{1}>, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {2}_dsl::{0}
            .order({2}_dsl::id.asc())
            .load::<{1}>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_all"))
    }}

    pub async fn get_by_id(id: i32, tenant_name: &str) -> Result<{1}, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {2}_dsl::{0}
            .filter({2}_dsl::id.eq(id))
            .first::<{1}>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "get_by_id").with_context("id", id.to_string()))
    }}

    pub async fn create(new_record: New{1}, tenant_name: &str) -> Result<{1}, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        diesel::insert_into({2}_dsl::{0})
            .values(&new_record)
            .get_result::<{1}>(&mut conn)
            .await
            .map_err(|e| MeltDown::from(e).with_context("operation", "create"))
    }}

    pub async fn update_by_id(id: i32, updates: &New{1}, tenant_name: &str) -> Result<{1}, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        diesel::update({2}_dsl::{0}.filter({2}_dsl::id.eq(id)))
            .set(updates)
            .get_result::<{1}>(&mut conn)
            .await
            .map_err(|e| MeltDown::from(e).with_context("operation", "update_by_id").with_context("id", id.to_string()))
    }}

    pub async fn delete_by_id(id: i32, tenant_name: &str) -> Result<(), MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        conn.transaction::<_, MeltDown, _>(|conn| {{
            async move {{
                {2}_dsl::{0}
                    .filter({2}_dsl::id.eq(id))
                    .first::<{1}>(conn)
                    .await?;

                diesel::delete({2}_dsl::{0}.filter({2}_dsl::id.eq(id)))
                    .execute(conn)
                    .await?;

                Ok(())
            }}
            .scope_boxed()
        }})
        .await
        .map_err(|e| MeltDown::from(e).with_context("operation", "delete_by_id").with_context("id", id.to_string()))
    }}

    pub async fn count(tenant_name: &str) -> Result<i64, MeltDown> {{
        let mut conn = establish_connection_with_tenant(tenant_name).await?;

        {2}_dsl::{0}
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e: diesel::result::Error| MeltDown::from(e).with_context("operation", "count"))
    }}{3}{4}{5}
}}
"#,
        table_name, struct_name, singular_name, bool_methods, timestamp_methods, relationship_methods
    );

    if let Err(e) = fs::write(&file_path, model_template) {
        if let Err(log_err) = crate::logger::error(&format!("Error writing model file {}: {}", file_path, e)) {
            eprintln!("logger error failed: {}", log_err);
        }
        false
    } else {
        true
    }
}

fn update_mod_file(_config: &Config, processed_tables: &[String]) -> bool {
    if processed_tables.is_empty() {
        return true;
    }

    let output_dir = "src/models/generated";

    let mod_file_path = Path::new(output_dir).join("mod.rs");
    let mut mod_file_content = if mod_file_path.exists() {
        match fs::read_to_string(&mod_file_path) {
            Ok(content) => content,
            Err(e) => {
                if let Err(log_err) = crate::logger::error(&format!("Error reading mod.rs file: {}", e)) {
                    eprintln!("logger error failed: {}", log_err);
                }
                return false;
            }
        }
    } else {
        String::new()
    };

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
            if let Err(log_err) = crate::logger::error(&format!("Error writing mod.rs file: {}", e)) {
                eprintln!("logger error failed: {}", log_err);
            }
            return false;
        }
    }

    true
}

pub fn generate(config: &Config) -> bool {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Generating enhanced model implementations...");

    let schema_path = "src/database/schema.rs";

    if !Path::new(schema_path).exists() {
        progress.error(&format!("Schema file not found at {}", schema_path));
        return false;
    }

    let ignore_list: Vec<String> = Vec::new();

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

    let relationships = match load_schema_relationships(schema_path) {
        Ok(rels) => rels,
        Err(e) => {
            if let Err(log_err) = crate::logger::warning(&format!("Error loading relationship information: {}. Continuing without relationship methods.", e)) {
                eprintln!("logger warning failed: {}", log_err);
            }
            Vec::new()
        }
    };

    let mut processed_tables = Vec::new();

    for table in &tables {
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
        if update_mod_file(config, &processed_tables) {
            progress.success(&format!("Generated {} enhanced model files with specialized methods", processed_tables.len()));
            true
        } else {
            progress.error("Failed to update mod.rs file");
            false
        }
    }
}
