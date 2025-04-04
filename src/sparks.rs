use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::configs::Config;
use crate::logger;

// Function to run migrations from a spark plugin
fn run_spark_migration(migration_path: &PathBuf) -> Result<(), String> {
    use std::process::{Command, Stdio};
    use dotenv::dotenv;
    
    // Load environment variables from .env file to ensure they're available to migrations
    dotenv().ok();
    
    logger::info(&format!("Running migrations from path: {}", migration_path.display()))?;
    
    // Check if migrations directory exists
    if !migration_path.exists() || !migration_path.is_dir() {
        return Err(format!("Migration path does not exist: {}", migration_path.display()));
    }
    
    // Create a progress tracker
    let mut progress = logger::create_progress(None);
    progress.set_message(&format!("Running migrations from: {}", migration_path.display()));
    
    // Run diesel migration with the custom path
    // Pass all environment variables from the current process
    let output = Command::new("diesel")
        .args(["migration", "run", "--migration-dir", &migration_path.to_string_lossy()])
        .envs(std::env::vars()) // Pass all environment variables
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to execute diesel migration: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
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
    } else if stdout.lines().next().is_some() {
        progress.success("Migrations completed successfully (no details available)");
    } else {
        progress.success("No migrations to run");
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
    let toml_content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read Catalyst.toml: {}", e))?;
    
    // Parse the content as a document to preserve formatting and comments
    let mut doc = toml_content.parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Failed to parse Catalyst.toml: {}", e))?;

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
    fs::write(config_path, updated_content)
        .map_err(|e| format!("Failed to write updated Catalyst.toml: {}", e))?;

    logger::info(&format!("Updated Catalyst.toml with spark: {} -> {}", spark_name, repo_url))?;
    Ok(true)
}

// Main function to add a spark plugin
pub fn add_spark(repo_url: &str, _config: &Config) -> Result<(), String> {
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
        fs::create_dir_all(&sparks_dir)
            .map_err(|e| format!("Failed to create sparks directory: {}", e))?;
        
        // If the module file doesn't exist, create it with a basic comment
        let mod_rs_path = sparks_dir.join("mod.rs");
        if !mod_rs_path.exists() {
            fs::write(&mod_rs_path, "//here you include the modules you want to expose to the outside world\n")
                .map_err(|e| format!("Failed to create mod.rs file: {}", e))?;
        }
    }
    
    // Step 2: Create a temporary directory for cloning
    let temp_dir = format!("_temp_spark_{}", repo_name);
    let temp_path = Path::new(&temp_dir);
    
    // Clean up any existing temporary directory
    if temp_path.exists() {
        fs::remove_dir_all(temp_path)
            .map_err(|e| format!("Failed to clean up temporary directory: {}", e))?;
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
        fs::remove_dir_all(&target_dir)
            .map_err(|e| format!("Failed to remove existing spark directory: {}", e))?;
    }
    
    // Copy the repository to the sparks directory
    copy_dir_all(temp_path, &target_dir)
        .map_err(|e| format!("Failed to copy spark to target directory: {}", e))?;
    
    // Clean up: Remove the temporary directory
    fs::remove_dir_all(temp_path)
        .map_err(|e| format!("Failed to clean up temporary directory: {}", e))?;
    
    // Step 6: Update the mod.rs file to include the new spark
    update_sparks_mod_rs(&sparks_dir, &repo_name)?;
    
    // Step 7: Update the project's Cargo.toml with any required dependencies
    if !validation_result.dependencies.is_empty() {
        progress.set_message("Updating Cargo.toml with dependencies...");
        
        // Log dependency information for debugging
        logger::info(&format!("Found {} dependencies in spark manifest", validation_result.dependencies.len()))?;
        for dep in &validation_result.dependencies {
            let features_str = if dep.features.is_empty() { 
                String::new() 
            } else { 
                format!(" with features: {}", dep.features.join(", ")) 
            };
            
            let version_str = if let Some(version) = &dep.version {
                format!(" v{}", version)
            } else {
                String::new()
            };
            
            logger::info(&format!("  - {}{}{}", dep.crate_name, version_str, features_str))?;
        }
        
        update_cargo_toml(&validation_result.dependencies)?;
    }
    
    // Step 8: Check for required environment variables and update .env if needed
    let env_updated = if !validation_result.required_env.is_empty() {
        // Add env variables
        progress.set_message(&format!("Setting up environment variables for: {}", validation_result.name));
        update_env_variables(&repo_name, &validation_result.required_env)?
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
            
            progress.set_message(&format!("Running migration: {}", migration.name));
            match run_spark_migration(&migration_path) {
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
            logger::info(&format!("  - {} {}", dep.crate_name, if !dep.features.is_empty() { format!("with features: {}", dep.features.join(", ")) } else { String::new() }))?;
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
    let mut file = fs::File::open(manifest_path)
        .map_err(|e| format!("Failed to open manifest file: {}", e))?;
    
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| format!("Failed to read manifest file: {}", e))?;
    
    // Parse the TOML
    let parsed = content.parse::<toml::Value>()
        .map_err(|e| format!("Failed to parse manifest TOML: {}", e))?;
    
    // Validate [spark] section (mandatory)
    let spark = parsed.get("spark")
        .ok_or_else(|| format!("Missing [spark] section in manifest"))?
        .as_table()
        .ok_or_else(|| format!("[spark] section must be a table"))?;
    
    // Extract mandatory fields from [spark]
    let name = spark.get("name")
        .ok_or_else(|| format!("Missing name in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("name must be a string"))?
        .to_string();
    
    let version = spark.get("version")
        .ok_or_else(|| format!("Missing version in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("version must be a string"))?
        .to_string();
    
    let description = spark.get("description")
        .ok_or_else(|| format!("Missing description in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("description must be a string"))?
        .to_string();
    
    let author = spark.get("author")
        .ok_or_else(|| format!("Missing author in [spark] section"))?
        .as_str()
        .ok_or_else(|| format!("author must be a string"))?
        .to_string();
    
    let license = spark.get("license")
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
                            let crate_name = feature_table.get("crate_name")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| format!("Missing crate_name in dependency"))?
                                .to_string();
                            
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
                    let migration_name = migration_table.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unnamed")
                        .to_string();
                    
                    let migration_path = migration_table.get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("migrations")
                        .to_string();
                    
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
    let url = url::Url::parse(repo_url)
        .map_err(|e| format!("Invalid URL: {}", e))?;
    
    let path = url.path().trim_end_matches('/');
    let name = path.split('/').last()
        .ok_or_else(|| format!("Could not extract repository name from URL"))?;
    
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
    let cargo_content = fs::read_to_string(cargo_path)
        .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;
    
    // Parse Cargo.toml
    let mut cargo_doc = cargo_content.parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;
    
    // Get or create the dependencies table
    let deps_table = if cargo_doc.contains_key("dependencies") {
        // Table exists, get mutable reference
        cargo_doc["dependencies"].as_table_mut()
            .ok_or_else(|| format!("Failed to access dependencies table in Cargo.toml"))?
    } else {
        // Table doesn't exist, create it
        cargo_doc["dependencies"] = toml_edit::Item::Table(toml_edit::Table::new());
        cargo_doc["dependencies"].as_table_mut()
            .ok_or_else(|| format!("Failed to create dependencies table in Cargo.toml"))?
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
                logger::info(&format!("Dependency {} found with type: {}", dep_name, 
                    if existing_dep.is_str() { "string" } 
                    else if existing_dep.is_table() { "table" }
                    else if existing_dep.is_inline_table() { "inline_table" }
                    else { "other" }))?;
                    
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
            },
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
    let env_content = fs::read_to_string(env_path)
        .map_err(|e| format!("Failed to read .env file: {}", e))?;
    
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
            let spark_vars: Vec<_> = placeholder_vars.iter()
                .filter(|v| v.starts_with(&spark_prefix))
                .collect();
                
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
                fs::write(env_path, all_lines.join("\n"))
                    .map_err(|e| format!("Failed to update .env file: {}", e))?;
            }
        }
        
        // Now append the new variables
        let mut env_file = fs::OpenOptions::new()
            .append(true)
            .open(env_path)
            .map_err(|e| format!("Failed to open .env file for writing: {}", e))?;
        
        // Add a comment indicating these are for the spark
        writeln!(env_file, "\n# Environment variables for {} spark", spark_name)
            .map_err(|e| format!("Failed to write to .env file: {}", e))?;
        
        // Add each variable with a placeholder value
        for (name, comment) in &vars_to_update {
            // Write as NAME="YOUR_VALUE_HERE" format with comment if available
            let line = if comment.is_empty() {
                format!("{}=\"REPLACE_THIS_WITH_YOUR_VALUE\"", name)
            } else {
                format!("{}=\"REPLACE_THIS_WITH_YOUR_VALUE\" {}", name, comment)
            };
            
            writeln!(env_file, "{}", line)
                .map_err(|e| format!("Failed to write to .env file: {}", e))?;
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
            std::thread::sleep(std::time::Duration::from_secs(1));
            
            match Command::new(&editor).arg(env_path).status() {
                Ok(status) if status.success() => {
                    println!("\nFile has been successfully edited.");
                    true
                },
                _ => {
                    println!("\nCould not open editor from EDITOR environment variable.");
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
        let updated_content = fs::read_to_string(env_path)
            .map_err(|e| format!("Failed to read .env file after editing: {}", e))?;
        
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
            println!("\n⚠️  Warning: Some environment variables for {} still have placeholder values!", spark_name);
            println!("Please manually edit the .env file at: {}", env_path.display());
            println!("Replace the REPLACE_THIS_WITH_YOUR_VALUE placeholders with actual values.");
        } else {
            println!("\n✅ All environment variables have been set.");
        }
    } else {
        println!("\n✅ All required environment variables are already set.");
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
    
    #[cfg(target_os = "windows")]
    {
        // On Windows, try notepad
        if let Ok(status) = Command::new("notepad")
            .arg(file_path)
            .status() 
        {
            if status.success() {
                println!("\nFile has been edited in notepad.");
                return Ok(());
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // On Unix-like systems, try various common editors
        let editors = ["nano", "vim", "vi", "gedit", "code", "emacs", "sublime", "pico"];
        
        for editor in editors {
            // Try to check if the editor is installed
            if Command::new("which").arg(editor).output().map(|o| o.status.success()).unwrap_or(false) {
                // Editor exists, try to use it
                if let Ok(status) = Command::new(editor)
                    .arg(file_path)
                    .status() 
                {
                    if status.success() {
                        println!("\nFile has been edited with {}.", editor);
                        return Ok(());
                    }
                }
            }
        }
    }
    
    // If we got here, we couldn't open any editor
    println!("\n⚠️  Could not find a suitable editor to open the .env file");
    println!("Please manually edit the file at: {}", file_path.display());
    Ok(())
}

// Function to check for and install sparks in Catalyst.toml
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
    let mut content = fs::read_to_string(&mod_rs_path)
        .map_err(|e| format!("Failed to read mod.rs file: {}", e))?;
    
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
        fs::write(&mod_rs_path, content)
            .map_err(|e| format!("Failed to update mod.rs file: {}", e))?;
    }
    
    Ok(())
}