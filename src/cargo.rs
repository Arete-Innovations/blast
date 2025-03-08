use crate::configs::Config;
use crate::progress::ProgressManager;
use dialoguer::{theme::ColorfulTheme, FuzzySelect, Input, MultiSelect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use toml::{Table, Value};

// Crates.io API response structure (simplified)
#[derive(Debug, Deserialize)]
struct CratesResponse {
    crates: Vec<Crate>,
}

#[derive(Debug, Deserialize)]
struct Crate {
    name: String,
    max_version: String,
    description: Option<String>,
    downloads: u64,
}

// Fuzzy-search crates.io and return list of matching crates
async fn search_crates(query: &str) -> Result<Vec<Crate>, Box<dyn Error>> {
    let progress = ProgressManager::new_spinner();
    let msg = format!("Searching crates.io for '{}'...", query);
    progress.set_message(&msg);

    let client = reqwest::Client::new();
    let url = format!("https://crates.io/api/v1/crates?q={}&per_page=50", query);
    
    let response = client.get(&url)
        .header("User-Agent", "blast/0.1.0")
        .send()
        .await?;
    
    if response.status().is_success() {
        let crates_response: CratesResponse = response.json().await?;
        if crates_response.crates.is_empty() {
            progress.error("No crates found matching your query");
            return Err("No matching crates found".into());
        }
        
        let success_msg = format!("Found {} matching crates", crates_response.crates.len());
        progress.success(&success_msg);
        Ok(crates_response.crates)
    } else {
        progress.error("Failed to search crates.io");
        Err(format!("API request failed: {}", response.status()).into())
    }
}

// Format crate choices for dialoguer selection
fn format_crate_choices(crates: &[Crate]) -> Vec<String> {
    crates.iter()
        .map(|c| {
            let desc = c.description.as_ref()
                .map(|d| if d.len() > 60 { format!("{}...", &d[..57]) } else { d.clone() })
                .unwrap_or_else(|| "No description".to_string());
            
            format!("{} v{} ({} downloads) - {}", 
                c.name, 
                c.max_version, 
                format_downloads(c.downloads),
                desc
            )
        })
        .collect()
}

// Format download numbers (e.g., 1234567 -> 1.2M)
fn format_downloads(downloads: u64) -> String {
    if downloads >= 1_000_000 {
        format!("{:.1}M", downloads as f64 / 1_000_000.0)
    } else if downloads >= 1_000 {
        format!("{:.1}K", downloads as f64 / 1_000.0)
    } else {
        downloads.to_string()
    }
}

// Parse Cargo.toml and get current dependencies
fn get_current_dependencies(config: &Config) -> Result<Table, Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let cargo_path = project_dir.join("Cargo.toml");
    
    if !cargo_path.exists() {
        return Err("Cargo.toml not found".into());
    }
    
    let cargo_content = fs::read_to_string(cargo_path)?;
    let cargo_toml: Table = toml::from_str(&cargo_content)?;
    
    // Get the dependencies table
    if let Some(Value::Table(deps)) = cargo_toml.get("dependencies") {
        Ok(deps.clone())
    } else {
        Ok(Table::new())
    }
}

// Add a dependency to Cargo.toml
fn add_dependency_to_cargo_toml(config: &Config, name: &str, version: &str) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let cargo_path = project_dir.join("Cargo.toml");
    
    if !cargo_path.exists() {
        return Err("Cargo.toml not found".into());
    }
    
    // Read the current Cargo.toml content
    let cargo_content = fs::read_to_string(&cargo_path)?;
    let mut cargo_toml: Table = toml::from_str(&cargo_content)?;
    
    // Get or create the dependencies table
    let deps = if let Some(Value::Table(deps)) = cargo_toml.get_mut("dependencies") {
        deps
    } else {
        cargo_toml.insert("dependencies".to_string(), Value::Table(Table::new()));
        if let Some(Value::Table(deps)) = cargo_toml.get_mut("dependencies") {
            deps
        } else {
            return Err("Failed to create dependencies table".into());
        }
    };
    
    // Add the new dependency
    deps.insert(name.to_string(), Value::String(version.to_string()));
    
    // Write the updated Cargo.toml
    let updated_content = toml::to_string(&cargo_toml)?;
    fs::write(cargo_path, updated_content)?;
    
    Ok(())
}

// Remove dependencies from Cargo.toml
fn remove_dependencies_from_cargo_toml(config: &Config, names: &[String]) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let cargo_path = project_dir.join("Cargo.toml");
    
    if !cargo_path.exists() {
        return Err("Cargo.toml not found".into());
    }
    
    // Read the current Cargo.toml content
    let cargo_content = fs::read_to_string(&cargo_path)?;
    let mut cargo_toml: Table = toml::from_str(&cargo_content)?;
    
    // Get the dependencies table
    if let Some(Value::Table(deps)) = cargo_toml.get_mut("dependencies") {
        // Remove the specified dependencies
        for name in names {
            deps.remove(name);
        }
        
        // Write the updated Cargo.toml
        let updated_content = toml::to_string(&cargo_toml)?;
        fs::write(cargo_path, updated_content)?;
        
        Ok(())
    } else {
        Err("No dependencies found in Cargo.toml".into())
    }
}

// Main function to add a dependency - handles searching and selection
pub async fn add_dependency(config: &Config, search_term: &str) -> Result<(), Box<dyn Error>> {
    // If no search term provided, prompt for one
    let search = if search_term.is_empty() {
        let search: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter search term")
            .interact_text()?;
        search
    } else {
        search_term.to_string()
    };
    
    if search.is_empty() {
        return Err("No search term provided".into());
    }
    
    // Search crates.io
    let crates = search_crates(&search).await?;
    let choices = format_crate_choices(&crates);
    
    // Present results for selection
    println!("Select a crate to add:");
    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select crate")
        .default(0)
        .items(&choices)
        .interact()?;
    
    let selected_crate = &crates[selection];
    
    // Confirm version
    let version = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Version (default: {})", selected_crate.max_version))
        .default(selected_crate.max_version.clone())
        .interact_text()?;
    
    // Add to Cargo.toml
    let progress = ProgressManager::new_spinner();
    let add_msg = format!("Adding {} v{} to Cargo.toml...", selected_crate.name, version);
    progress.set_message(&add_msg);
    
    match add_dependency_to_cargo_toml(config, &selected_crate.name, &version) {
        Ok(_) => {
            let success_msg = format!("Added {} v{} to Cargo.toml", selected_crate.name, version);
            progress.success(&success_msg);
            
            // Run cargo update
            progress.set_message("Running cargo update...");
            match std::process::Command::new("cargo")
                .args(["update", "-p", &selected_crate.name])
                .current_dir(&config.project_dir)
                .output() {
                Ok(_) => {
                    progress.success("Dependency added and updated successfully");
                }
                Err(e) => {
                    let err_msg = format!("Dependency added but update failed: {}", e);
                    progress.error(&err_msg);
                }
            }
            
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to add dependency: {}", e);
            progress.error(&err_msg);
            Err(e)
        }
    }
}

// Main function to remove dependencies - handles selection
pub fn remove_dependency(config: &Config) -> Result<(), Box<dyn Error>> {
    // Get current dependencies
    let deps = get_current_dependencies(config)?;
    
    if deps.is_empty() {
        println!("No dependencies found in Cargo.toml");
        return Ok(());
    }
    
    // Format dependency choices
    let dep_names: Vec<String> = deps.keys().cloned().collect();
    let dep_choices: Vec<String> = dep_names.iter()
        .map(|name| {
            let version = match deps.get(name) {
                Some(Value::String(v)) => v.clone(),
                Some(Value::Table(t)) => {
                    if let Some(Value::String(v)) = t.get("version") {
                        v.clone()
                    } else {
                        "complex dependency".to_string()
                    }
                }
                _ => "unknown version".to_string(),
            };
            format!("{} ({})", name, version)
        })
        .collect();
    
    // Present dependencies for selection
    println!("Select dependencies to remove (space to select, enter to confirm):");
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select dependencies to remove")
        .items(&dep_choices)
        .interact()?;
    
    if selections.is_empty() {
        println!("No dependencies selected for removal");
        return Ok(());
    }
    
    // Get selected dependency names
    let selected_deps: Vec<String> = selections.iter()
        .map(|&i| dep_names[i].clone())
        .collect();
    
    // Confirm removal
    println!("You are about to remove these dependencies:");
    for dep in &selected_deps {
        println!("  - {}", dep);
    }
    
    let proceed = dialoguer::Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed with removal?")
        .default(false)
        .interact()?;
    
    if !proceed {
        println!("Operation cancelled");
        return Ok(());
    }
    
    // Remove from Cargo.toml
    let progress = ProgressManager::new_spinner();
    let remove_msg = format!("Removing {} dependencies from Cargo.toml...", selected_deps.len());
    progress.set_message(&remove_msg);
    
    match remove_dependencies_from_cargo_toml(config, &selected_deps) {
        Ok(_) => {
            let success_msg = format!("Removed {} dependencies from Cargo.toml", selected_deps.len());
            progress.success(&success_msg);
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to remove dependencies: {}", e);
            progress.error(&err_msg);
            Err(e)
        }
    }
}