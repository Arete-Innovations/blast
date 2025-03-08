use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use toml::Value;

#[derive(Clone)]
pub struct Config {
    pub environment: String,
    pub project_name: String,
    pub assets: Value,
    pub project_dir: PathBuf,
    pub show_compiler_warnings: bool,
}

// For interactive mode: loads files from current directory.
pub fn get_project_info() -> Result<Config, Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let config_path = format!("{}/Catalyst.toml", cwd.display());
    get_project_info_with_paths(&config_path, &cwd)
}

// Toggle the environment between dev and prod
// Launch Catalyst.toml configuration manager
pub fn launch_manager(config: &mut Config) {
    use console::style;
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};
    use std::io::Write;

    // Use ANSI clear screen to clean any existing output
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().unwrap();

    loop {
        // Build dynamic options with current values
        let mut display_options = Vec::new();

        // [MAJOR] Project settings
        display_options.push(style("[PROJECT] Settings").bold().to_string());

        // Environment with current value
        let env_status = if config.environment == "dev" { style("dev").green() } else { style("prod").red() };
        display_options.push(format!("[SETTING] Environment: {}", env_status));

        // Compiler warnings with current value
        let warnings_status = if config.show_compiler_warnings { style("enabled").green() } else { style("disabled").yellow() };
        display_options.push(format!("[SETTING] Compiler Warnings: {}", warnings_status));

        // Project name
        display_options.push(format!("[SETTING] Project Name: {}", style(&config.project_name).cyan()));

        // [MAJOR] Git settings
        display_options.push(style("[GIT] Configuration").bold().to_string());

        // Git settings
        let config_path = config.project_dir.join("Catalyst.toml");
        let toml_content = match fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(_) => String::new(),
        };

        let toml_value: toml::Value = match toml_content.parse() {
            Ok(value) => value,
            Err(_) => toml::Value::Table(toml::value::Table::new()),
        };

        // Git settings status
        let git_status = if toml_value.get("git").is_some() {
            let username = toml_value.get("git").and_then(|g| g.get("username")).and_then(|u| u.as_str()).unwrap_or("");

            if !username.is_empty() {
                style("configured").green()
            } else {
                style("incomplete").yellow()
            }
        } else {
            style("not configured").yellow()
        };
        display_options.push(format!("[SETTING] Git Configuration: {}", git_status));

        // [MAJOR] Other options
        display_options.push(style("[OTHER] Actions").bold().to_string());
        display_options.push("[ACTION] View Current Configuration".to_string());
        display_options.push("[ACTION] Add/Edit Custom Setting".to_string());
        display_options.push(style("[EXIT] Back to Main Menu").dim().to_string());

        // Show the menu with current values
        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Catalyst.toml Configuration")
            .items(&display_options)
            .default(0)
            .interact()
            .unwrap();

        // Process selection
        match display_options[selection].as_str() {
            s if s.contains("[PROJECT] Settings") => continue,  // This is a heading
            s if s.contains("[GIT] Configuration") => continue, // This is a heading
            s if s.contains("[OTHER] Actions") => continue,     // This is a heading
            s if s.contains("Environment:") => {
                // Toggle environment with a single press
                let old_env = config.environment.clone();
                match toggle_environment(config) {
                    Ok(_) => {
                        println!("✅ Environment changed from {} to {}", old_env, config.environment);
                    }
                    Err(e) => {
                        println!("❌ Error toggling environment: {}", e);
                    }
                }
            }
            s if s.contains("Compiler Warnings:") => {
                // Toggle compiler warnings with a single press
                match toggle_compiler_warnings(config) {
                    Ok(_) => {
                        let status = if config.show_compiler_warnings { "enabled" } else { "disabled" };
                        println!("✅ Compiler warnings are now {}", status);
                    }
                    Err(e) => {
                        println!("❌ Error toggling compiler warnings: {}", e);
                    }
                }
            }
            s if s.contains("Project Name:") => edit_project_name(config),
            s if s.contains("Git Configuration:") => edit_git_config(config),
            s if s.contains("View Current Configuration") => view_current_config(config),
            s if s.contains("Add/Edit Custom Setting") => edit_custom_setting(config),
            s if s.contains("Back to Main Menu") => break,
            _ => continue,
        }

        // Wait for a key press before continuing
        println!("\nPress Enter to continue...");
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();

        // Clear screen for next operation
        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush().unwrap();
    }
}

// Edit Git configuration settings in Catalyst.toml
fn edit_git_config(config: &mut Config) {
    use console::style;
    use dialoguer::{theme::ColorfulTheme, FuzzySelect, Input};
    use std::io::Write;

    // Use ANSI clear screen to clean any existing output
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().unwrap();

    println!("{}", style("⚙️ Git Configuration").bold());

    // Read current Catalyst.toml content
    let config_path = config.project_dir.join("Catalyst.toml");
    let toml_content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            println!("❌ Error reading Catalyst.toml: {}", e);
            return;
        }
    };

    // Parse TOML
    let mut toml_value: toml::Value = match toml_content.parse() {
        Ok(value) => value,
        Err(e) => {
            println!("❌ Error parsing Catalyst.toml: {}", e);
            return;
        }
    };

    // Check if git section exists, if not create it
    if !toml_value.as_table().unwrap().contains_key("git") {
        println!("{}", style("[GIT] Section not found in Catalyst.toml").yellow());

        // Auto create the section instead of asking
        let mut git_table = toml::value::Table::new();

        // Add default values
        git_table.insert("remote_url".to_string(), toml::Value::String("".to_string()));
        git_table.insert("username".to_string(), toml::Value::String("".to_string()));
        git_table.insert("email".to_string(), toml::Value::String("".to_string()));

        // Update the TOML
        toml_value.as_table_mut().unwrap().insert("git".to_string(), toml::Value::Table(git_table));

        println!("✅ Created [git] section in Catalyst.toml");
    }

    // Get current values for display
    let mut current_remote = toml_value.get("git").and_then(|g| g.get("remote_url")).and_then(|u| u.as_str()).unwrap_or("").to_string();

    let mut current_username = toml_value.get("git").and_then(|g| g.get("username")).and_then(|u| u.as_str()).unwrap_or("").to_string();

    let mut current_email = toml_value.get("git").and_then(|g| g.get("email")).and_then(|u| u.as_str()).unwrap_or("").to_string();

    // Create option menu for Git settings
    loop {
        // Build dynamic options with current values
        let mut options = Vec::new();

        // Add options with current values
        options.push(format!(
            "[SETTING] Remote URL: {}",
            if current_remote.is_empty() { style("<not set>").dim() } else { style(current_remote.as_str()).cyan() }
        ));

        options.push(format!(
            "[SETTING] Username: {}",
            if current_username.is_empty() { style("<not set>").dim() } else { style(current_username.as_str()).green() }
        ));

        options.push(format!(
            "[SETTING] Email: {}",
            if current_email.is_empty() { style("<not set>").dim() } else { style(current_email.as_str()).green() }
        ));

        options.push(format!("{}", style("[ACTION] Apply to Git Config").bold().green()));
        options.push(format!("{}", style("[EXIT] Save and Return").dim()));

        // Show the menu with current values
        let selection = FuzzySelect::with_theme(&ColorfulTheme::default()).with_prompt("Git Configuration").items(&options).default(0).interact().unwrap();

        match selection {
            0 => {
                // Remote URL
                let new_remote: String = Input::new().with_prompt("Remote repository URL").default(current_remote.clone()).interact_text().unwrap();

                // Update local variable for display
                if new_remote != current_remote {
                    toml_value["git"].as_table_mut().unwrap().insert("remote_url".to_string(), toml::Value::String(new_remote.clone()));
                    current_remote = new_remote;

                    // Save immediately for responsive feedback
                    let formatted_toml = toml::to_string_pretty(&toml_value).unwrap_or_default();
                    let _ = fs::write(&config_path, formatted_toml);
                    println!("✅ Remote URL updated");
                }
            }
            1 => {
                // Username
                let new_username: String = Input::new().with_prompt("Git username").default(current_username.clone()).interact_text().unwrap();

                // Update local variable for display
                if new_username != current_username {
                    toml_value["git"].as_table_mut().unwrap().insert("username".to_string(), toml::Value::String(new_username.clone()));
                    current_username = new_username;

                    // Save immediately for responsive feedback
                    let formatted_toml = toml::to_string_pretty(&toml_value).unwrap_or_default();
                    let _ = fs::write(&config_path, formatted_toml);
                    println!("✅ Username updated");
                }
            }
            2 => {
                // Email
                let new_email: String = Input::new().with_prompt("Git email").default(current_email.clone()).interact_text().unwrap();

                // Update local variable for display
                if new_email != current_email {
                    toml_value["git"].as_table_mut().unwrap().insert("email".to_string(), toml::Value::String(new_email.clone()));
                    current_email = new_email;

                    // Save immediately for responsive feedback
                    let formatted_toml = toml::to_string_pretty(&toml_value).unwrap_or_default();
                    let _ = fs::write(&config_path, formatted_toml);
                    println!("✅ Email updated");
                }
            }
            3 => {
                // Apply to Git Config
                println!("{}", style("Applying settings to Git configuration...").cyan());

                // Set username
                if !current_username.trim().is_empty() {
                    match std::process::Command::new("git").args(["config", "user.name", &current_username]).output() {
                        Ok(_) => println!("✅ Set git user.name to {}", style(&current_username).green()),
                        Err(e) => println!("❌ Failed to set git user.name: {}", e),
                    }
                }

                // Set email
                if !current_email.trim().is_empty() {
                    match std::process::Command::new("git").args(["config", "user.email", &current_email]).output() {
                        Ok(_) => println!("✅ Set git user.email to {}", style(&current_email).green()),
                        Err(e) => println!("❌ Failed to set git user.email: {}", e),
                    }
                }

                // Set remote URL if not empty
                if !current_remote.trim().is_empty() {
                    // Check if remote origin exists
                    let remote_check = std::process::Command::new("git").args(["remote"]).output().unwrap_or_else(|_| std::process::Output {
                        status: std::process::ExitStatus::from_raw(1),
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    });

                    let remotes = String::from_utf8_lossy(&remote_check.stdout);

                    if remotes.contains("origin") {
                        // Set new URL for existing origin
                        match std::process::Command::new("git").args(["remote", "set-url", "origin", &current_remote]).output() {
                            Ok(_) => println!("✅ Updated git remote 'origin' URL to {}", style(&current_remote).cyan()),
                            Err(e) => println!("❌ Failed to update git remote: {}", e),
                        }
                    } else {
                        // Add new origin
                        match std::process::Command::new("git").args(["remote", "add", "origin", &current_remote]).output() {
                            Ok(_) => println!("✅ Added git remote 'origin' with URL {}", style(&current_remote).cyan()),
                            Err(e) => println!("❌ Failed to add git remote: {}", e),
                        }
                    }
                }

                println!("✅ Git configuration applied");

                // Wait for a key press before continuing
                println!("\nPress Enter to continue...");
                let mut buffer = String::new();
                std::io::stdin().read_line(&mut buffer).unwrap();
            }
            4 => {
                // Save and return
                // Save changes with proper TOML formatting
                let formatted_toml = match toml::to_string_pretty(&toml_value) {
                    Ok(formatted) => formatted,
                    Err(e) => {
                        println!("❌ Error formatting TOML: {}", e);
                        return;
                    }
                };

                match fs::write(&config_path, formatted_toml) {
                    Ok(_) => println!("✅ Git configuration saved successfully"),
                    Err(e) => println!("❌ Error saving Catalyst.toml: {}", e),
                }

                return;
            }
            _ => continue,
        }
    }
}

// Edit project name in Catalyst.toml and Cargo.toml
fn edit_project_name(config: &mut Config) {
    use console::style;
    use dialoguer::{Confirm, Input};
    use std::io::Write;

    // Use ANSI clear screen to clean any existing output
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().unwrap();

    println!("{}", style("📝 Project Name Configuration").bold());

    // Display current project name with styling
    println!("Current project name: {}", style(&config.project_name).cyan());

    // Show warning about the implications of changing the project name
    println!("\n{}", style("⚠️  Warning").yellow().bold());
    println!("{}", style("Changing the project name will:").yellow());
    println!(" • Update the name in Cargo.toml");
    println!(" • Update the name in Catalyst.toml");
    println!(" • May require restarting the application");
    println!(" • Will not rename directories or source files");

    // Confirm if user wants to change project name
    let change_name = Confirm::new().with_prompt(style("Do you want to continue?").bold().to_string()).default(false).interact().unwrap();

    if !change_name {
        println!("Operation cancelled.");
        return;
    }

    // Ask for new project name
    let new_name: String = Input::new()
        .with_prompt(style("New project name").bold().to_string())
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.is_empty() {
                Err("Project name cannot be empty")
            } else if input.contains(char::is_whitespace) || !input.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                Err("Project name can only contain alphanumeric characters, underscores, and hyphens")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .unwrap();

    // Show progress indicator
    println!("\n{} {}", style("⏳").yellow(), style("Updating project files...").dim());

    // Update Cargo.toml
    let cargo_path = config.project_dir.join("Cargo.toml");
    let cargo_content = match fs::read_to_string(&cargo_path) {
        Ok(content) => content,
        Err(e) => {
            println!("{} Error reading Cargo.toml: {}", style("❌").red(), e);
            return;
        }
    };

    // Update project name in Cargo.toml
    let new_cargo_content = cargo_content.replace(&format!("name = \"{}\"", config.project_name), &format!("name = \"{}\"", new_name));

    // Update Catalyst.toml
    let catalyst_path = config.project_dir.join("Catalyst.toml");
    let catalyst_content = match fs::read_to_string(&catalyst_path) {
        Ok(content) => content,
        Err(e) => {
            println!("{} Error reading Catalyst.toml: {}", style("❌").red(), e);
            return;
        }
    };

    // Parse TOML to check if project name is defined
    let mut catalyst_toml: toml::Value = match catalyst_content.parse() {
        Ok(value) => value,
        Err(e) => {
            println!("{} Error parsing Catalyst.toml: {}", style("❌").red(), e);
            return;
        }
    };

    // Make sure the settings table exists
    if !catalyst_toml.as_table().unwrap().contains_key("settings") {
        catalyst_toml.as_table_mut().unwrap().insert("settings".to_string(), toml::Value::Table(toml::value::Table::new()));
    }

    // Update or add project name to settings
    catalyst_toml
        .as_table_mut()
        .unwrap()
        .get_mut("settings")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .insert("project_name".to_string(), toml::Value::String(new_name.clone()));

    // Format the TOML content
    let formatted_toml = match toml::to_string_pretty(&catalyst_toml) {
        Ok(formatted) => formatted,
        Err(e) => {
            println!("{} Error formatting TOML: {}", style("❌").red(), e);
            return;
        }
    };

    // Save changes
    match fs::write(&cargo_path, new_cargo_content) {
        Ok(_) => {
            match fs::write(&catalyst_path, formatted_toml) {
                Ok(_) => {
                    // Update config in memory
                    let old_name = config.project_name.clone();
                    config.project_name = new_name;

                    println!("{} Project name changed from {} to {}", style("✅").green(), style(old_name).dim(), style(&config.project_name).cyan().bold());
                    println!("{} All configuration files updated successfully", style("✅").green());
                    println!("\n{}", style("Note: You may need to restart the application for all changes to take effect.").italic());
                }
                Err(e) => {
                    println!("{} Error saving Catalyst.toml: {}", style("❌").red(), e);
                }
            }
        }
        Err(e) => {
            println!("{} Error saving Cargo.toml: {}", style("❌").red(), e);
        }
    }
}

// View current Catalyst.toml configuration with colored syntax highlighting
fn view_current_config(config: &Config) {
    use console::{style, Term};

    println!("{}", style("📄 Current Configuration").bold());

    // Read Catalyst.toml
    let config_path = config.project_dir.join("Catalyst.toml");
    match fs::read_to_string(&config_path) {
        Ok(content) => {
            let term = Term::stdout();
            let width = term.size().1 as usize;
            let separator = "=".repeat(width.min(60));

            println!("\n{}", style("Catalyst.toml").bold());
            println!("{}\n", style(separator).dim());

            // Display formatted TOML with syntax highlighting
            for line in content.lines() {
                if line.trim().starts_with('[') && line.trim().ends_with(']') {
                    // Section headers in bold blue
                    println!("{}", style(line).bold().blue());
                } else if line.contains('=') {
                    // Key-value pairs with colored parts
                    let parts: Vec<&str> = line.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        let key = parts[0].trim();
                        let value = parts[1].trim();

                        // Determine value color based on type
                        let colored_value = if value.starts_with('"') && value.ends_with('"') {
                            // String value in green
                            style(value).green()
                        } else if value == "true" || value == "false" {
                            // Boolean value in magenta
                            style(value).magenta()
                        } else if value.starts_with('[') {
                            // Array value in yellow
                            style(value).yellow()
                        } else if value.parse::<f64>().is_ok() {
                            // Numeric value in cyan
                            style(value).cyan()
                        } else {
                            // Other values in default color (no styling)
                            style(value)
                        };

                        println!("{} = {}", style(key).bold(), colored_value);
                    } else {
                        println!("{}", line);
                    }
                } else if !line.trim().is_empty() {
                    // Array items or other content with indentation preserved
                    if line.trim().starts_with('"') {
                        println!("{}", style(line).green());
                    } else {
                        println!("{}", line);
                    }
                } else {
                    // Empty lines
                    println!();
                }
            }
        }
        Err(e) => {
            println!("{} Error reading Catalyst.toml: {}", style("❌").red(), e);
        }
    }
}

// These format_toml and format_toml_value functions have been removed
// as they were unused. Instead, we use toml::to_string_pretty() for
// proper TOML formatting throughout the codebase.

// Add or edit a custom setting in Catalyst.toml
fn edit_custom_setting(config: &Config) {
    use dialoguer::{Input, Select};

    println!("⚙️ Custom Setting Editor");

    // Read current Catalyst.toml content
    let config_path = config.project_dir.join("Catalyst.toml");
    let toml_content = match fs::read_to_string(&config_path) {
        Ok(content) => content,
        Err(e) => {
            println!("❌ Error reading Catalyst.toml: {}", e);
            return;
        }
    };

    // Parse TOML
    let mut toml_value: toml::Value = match toml_content.parse() {
        Ok(value) => value,
        Err(e) => {
            println!("❌ Error parsing Catalyst.toml: {}", e);
            return;
        }
    };

    // Get all sections
    let sections = toml_value.as_table().unwrap().keys().map(|k| k.to_string()).collect::<Vec<String>>();

    // Add option to create a new section
    let mut all_options = sections.clone();
    all_options.push("Create new section".to_string());

    // Select section or create new
    let section_selection = Select::new().with_prompt("Select section or create new").items(&all_options).default(0).interact().unwrap();

    let section_name = if section_selection == sections.len() {
        // User chose to create new section
        let new_section: String = Input::new()
            .with_prompt("Enter new section name")
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.is_empty() {
                    Err("Section name cannot be empty")
                } else if input.contains(char::is_whitespace) || !input.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    Err("Section name can only contain alphanumeric characters and underscores")
                } else {
                    Ok(())
                }
            })
            .interact_text()
            .unwrap();

        // Create new section
        toml_value.as_table_mut().unwrap().insert(new_section.clone(), toml::Value::Table(toml::value::Table::new()));

        new_section
    } else {
        // User selected existing section
        sections[section_selection].clone()
    };

    // Get key name
    let key_name: String = Input::new()
        .with_prompt("Enter key name")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.is_empty() {
                Err("Key name cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .unwrap();

    // Get value type
    let value_types = vec!["String", "Integer", "Float", "Boolean"];
    let value_type_selection = Select::new().with_prompt("Select value type").items(&value_types).default(0).interact().unwrap();

    // Get value based on type
    let value = match value_types[value_type_selection] {
        "String" => {
            let input: String = Input::new().with_prompt("Enter string value").interact_text().unwrap();
            toml::Value::String(input)
        }
        "Integer" => {
            let input: String = Input::new()
                .with_prompt("Enter integer value")
                .validate_with(|input: &String| -> Result<(), &str> {
                    match input.parse::<i64>() {
                        Ok(_) => Ok(()),
                        Err(_) => Err("Invalid integer value"),
                    }
                })
                .interact_text()
                .unwrap();
            toml::Value::Integer(input.parse::<i64>().unwrap())
        }
        "Float" => {
            let input: String = Input::new()
                .with_prompt("Enter float value")
                .validate_with(|input: &String| -> Result<(), &str> {
                    match input.parse::<f64>() {
                        Ok(_) => Ok(()),
                        Err(_) => Err("Invalid float value"),
                    }
                })
                .interact_text()
                .unwrap();
            toml::Value::Float(input.parse::<f64>().unwrap())
        }
        "Boolean" => {
            let options = vec!["true", "false"];
            let selection = Select::new().with_prompt("Select boolean value").items(&options).default(0).interact().unwrap();
            toml::Value::Boolean(selection == 0)
        }
        _ => {
            println!("❌ Invalid value type");
            return;
        }
    };

    // Set value in toml
    toml_value.as_table_mut().unwrap().get_mut(&section_name).unwrap().as_table_mut().unwrap().insert(key_name.clone(), value);

    // Save changes with proper TOML formatting
    let formatted_toml = match toml::to_string_pretty(&toml_value) {
        Ok(formatted) => formatted,
        Err(e) => {
            println!("❌ Error formatting TOML: {}", e);
            return;
        }
    };

    match fs::write(&config_path, formatted_toml) {
        Ok(_) => {
            println!("✅ Added/updated key '{}' in section '{}' successfully", key_name, section_name);
        }
        Err(e) => {
            println!("❌ Error saving Catalyst.toml: {}", e);
        }
    }
}

pub fn toggle_environment(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    use console::style;

    // Toggle between dev and prod
    let old_env = config.environment.clone();
    config.environment = if config.environment == "dev" { "prod".to_string() } else { "dev".to_string() };

    // Update the config file using proper TOML parsing
    let config_path = config.project_dir.join("Catalyst.toml");
    let toml_content = fs::read_to_string(&config_path)?;

    // Parse the TOML content
    let mut parsed_toml: toml::Value = toml_content.parse()?;

    // Make sure the settings table exists
    if !parsed_toml.as_table().unwrap().contains_key("settings") {
        parsed_toml.as_table_mut().unwrap().insert("settings".to_string(), toml::Value::Table(toml::value::Table::new()));
    }

    // Update the environment setting
    parsed_toml
        .as_table_mut()
        .unwrap()
        .get_mut("settings")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .insert("environment".to_string(), toml::Value::String(config.environment.clone()));

    // Format the TOML content and write it back
    let formatted_toml = toml::to_string_pretty(&parsed_toml)?;
    fs::write(config_path, formatted_toml)?;

    // Print information about the change with colored output
    let old_env_style = if old_env == "dev" { style(old_env).green() } else { style(old_env).red() };
    let new_env_style = if config.environment == "dev" {
        style(&config.environment).green()
    } else {
        style(&config.environment).red()
    };

    println!("Environment changed from {} to {}", old_env_style, new_env_style);

    Ok(())
}

// Toggle compiler warnings setting
pub fn toggle_compiler_warnings(config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
    use console::style;

    // Toggle show_compiler_warnings
    let old_state = config.show_compiler_warnings;
    config.show_compiler_warnings = !config.show_compiler_warnings;

    // Update the config file
    let config_path = config.project_dir.join("Catalyst.toml");
    let toml_content = fs::read_to_string(&config_path)?;

    // Parse the TOML content
    let mut parsed_toml: toml::Value = toml_content.parse()?;

    // Make sure the settings table exists
    if !parsed_toml.as_table().unwrap().contains_key("settings") {
        parsed_toml.as_table_mut().unwrap().insert("settings".to_string(), toml::Value::Table(toml::value::Table::new()));
    }

    // Update the show_compiler_warnings setting
    parsed_toml
        .as_table_mut()
        .unwrap()
        .get_mut("settings")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .insert("show_compiler_warnings".to_string(), toml::Value::Boolean(config.show_compiler_warnings));

    // Format the TOML content and write it back
    let formatted_toml = toml::to_string_pretty(&parsed_toml)?;
    fs::write(config_path, formatted_toml)?;

    // Print information about the change with colored output
    let old_state_str = if old_state { "enabled" } else { "disabled" };
    let new_state_str = if config.show_compiler_warnings { "enabled" } else { "disabled" };

    let old_state_style = if old_state { style(old_state_str).green() } else { style(old_state_str).yellow() };
    let new_state_style = if config.show_compiler_warnings { style(new_state_str).green() } else { style(new_state_str).yellow() };

    println!("Compiler warnings changed from {} to {}", old_state_style, new_state_style);

    Ok(())
}

// For new projects: accepts an explicit project directory.
pub fn get_project_info_with_paths<P: AsRef<std::path::Path>>(config_path: &str, project_dir: P) -> Result<Config, Box<dyn std::error::Error>> {
    let config_str = fs::read_to_string(config_path)?;
    let config_val: Value = config_str.parse().expect("Invalid TOML");

    let cargo_toml_path = format!("{}/Cargo.toml", project_dir.as_ref().display());
    let cargo_str = fs::read_to_string(&cargo_toml_path)?;
    let cargo: Value = cargo_str.parse().expect("Invalid TOML");
    let project_name = cargo["package"]["name"].as_str().unwrap_or("Unknown").to_string();

    // Get show_compiler_warnings setting, default to true if not specified
    let show_compiler_warnings = config_val.get("settings").and_then(|s| s.get("show_compiler_warnings")).and_then(|v| v.as_bool()).unwrap_or(true);

    Ok(Config {
        environment: config_val["settings"]["environment"].as_str().unwrap_or("dev").to_string(),
        project_name,
        assets: config_val,
        project_dir: project_dir.as_ref().to_path_buf(),
        show_compiler_warnings,
    })
}
