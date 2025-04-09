use chrono;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use toml_edit::{value, DocumentMut};

// Template repository URLs (primary and fallbacks)
const TEMPLATE_REPOS: [&str; 3] = [
    "https://github.com/Arete-Innovations/catalyst.git",
    "https://gitlab.com/Arete-Innovations/catalyst.git", // Fallback 1
    "https://bitbucket.org/Arete-Innovations/catalyst.git", // Fallback 2
];

// Maximum time to wait for clone operation in seconds
const CLONE_TIMEOUT: Duration = Duration::from_secs(30);

fn generate_jwt_secret() -> String {
    use rand::Rng;
    
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                              abcdefghijklmnopqrstuvwxyz\
                              0123456789";
    const SECRET_LEN: usize = 32;
    let mut rng = rand::rng();

    (0..SECRET_LEN)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn create_new_project(project_name: &str) {
    use console::style;
    
    println!("{} project: {}", style("Creating new").green().bold(), style(project_name).cyan());
    let project_path = Path::new(project_name);
    
    if project_path.exists() {
        eprintln!("{} Directory {} already exists.", style("Error:").red().bold(), project_name);
        return;
    }
    
    // Create a temporary directory first
    let temp_dir = format!("{}_temp", project_name);
    let temp_path = Path::new(&temp_dir);
    
    // Clean up any existing temporary directory
    if temp_path.exists() {
        println!("{} Cleaning up temporary directory...", style("⚙").cyan());
        if let Err(e) = fs::remove_dir_all(temp_path) {
            eprintln!("{} Failed to clean up temporary directory: {}", style("Error:").red().bold(), e);
            return;
        }
    }
    
    // Create project structure in the temporary directory
    println!("{} Fetching project template...", style("📥").cyan());
    if let Err(e) = create_and_dump_template(temp_path) {
        eprintln!("{} Failed to create project structure: {}", style("Error:").red().bold(), e);
        
        // Clean up the temporary directory on failure
        if temp_path.exists() {
            let _ = fs::remove_dir_all(temp_path);
        }
        
        return;
    }
    
    // Rename the temporary directory to the target project name
    println!("{} Creating project directory...", style("📂").cyan());
    if let Err(e) = fs::rename(temp_path, project_path) {
        eprintln!("{} Failed to create project directory: {}", style("Error:").red().bold(), e);
        
        // Clean up the temporary directory on failure
        if temp_path.exists() {
            let _ = fs::remove_dir_all(temp_path);
        }
        
        return;
    }
    
    // Update project configuration (Cargo.toml, .env, etc.)
    println!("{} Configuring project...", style("⚙").cyan());
    if let Err(e) = update_project(project_path, project_name) {
        eprintln!("{} Failed to update project configuration: {}", style("Error:").red().bold(), e);
        return;
    }
    
    println!("\n{} Project {} created successfully! {}", 
        style("✅").green().bold(), 
        style(project_name).cyan().bold(),
        style("🚀").green().bold()
    );
    
    println!("\nNext steps:");
    println!("  {} Change to project directory: {}", style("▶").cyan(), style(format!("cd {}", project_name)).yellow());
    println!("  {} Initialize the project: {}", style("▶").cyan(), style("blast init").yellow().bold());
    println!("  {} Start the interactive dashboard: {}", style("▶").cyan(), style("blast dashboard").yellow());
    println!("  {} Run the development server: {}", style("▶").cyan(), style("blast serve").yellow());
}

fn create_and_dump_template(dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    
    // Try cloning from each repository in order until successful
    let mut clone_successful = false;
    let mut last_error = String::new();
    
    // Determine whether to show verbose output based on environment
    let is_verbose = std::env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1";
    
    for repo_url in TEMPLATE_REPOS.iter() {
        // Only show the attempting to clone message in verbose mode
        if is_verbose {
            println!("Attempting to clone template from: {}", repo_url);
        }
        
        // Prepare the command
        let mut cmd = Command::new("git");
        cmd.args(["clone", "--depth=1", "--single-branch", 
              "--branch", "master", 
              "--config", &format!("core.askPass=echo"),
              "--config", &format!("http.connectTimeout={}", CLONE_TIMEOUT.as_secs()),
              "--config", &format!("http.lowSpeedLimit=1000"),
              "--config", &format!("http.lowSpeedTime={}", CLONE_TIMEOUT.as_secs()),
              repo_url, &dest.to_string_lossy()]);
        
        // Hide output unless in verbose mode
        if !is_verbose {
            if cfg!(target_os = "windows") {
                cmd.stdin(std::process::Stdio::null())
                   .stdout(std::process::Stdio::null())
                   .stderr(std::process::Stdio::null());
            } else {
                cmd.arg("--quiet"); // Git's quiet flag
            }
        }
        
        // Run the command
        let status = cmd.status();
            
        match status {
            Ok(exit_status) if exit_status.success() => {
                clone_successful = true;
                
                // Only show success message in verbose mode
                if is_verbose {
                    println!("Successfully cloned template repository.");
                }
                
                // Remove the .git directory from the cloned repo
                let git_dir = dest.join(".git");
                if git_dir.exists() {
                    fs::remove_dir_all(git_dir)?;
                }
                
                break;
            },
            Ok(_) => {
                last_error = format!("Git clone command failed for repository: {}", repo_url);
                // Continue to the next repository
            },
            Err(e) => {
                last_error = format!("Failed to execute git clone: {}", e);
                // Continue to the next repository
            }
        }
    }
    
    if !clone_successful {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other, 
            format!("Failed to clone template from any repository. Last error: {}", last_error)
        ));
    }

    // Initialize log files with content to prevent race conditions with zellij
    let logs_dir = dest.join("storage").join("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(&logs_dir)?;
    }
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_files = ["debug.log", "error.log", "info.log", "server.log", "warning.log"];

    for log_file in log_files.iter() {
        let log_path = logs_dir.join(log_file);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&log_path)?;

        writeln!(file, "--- Log initialized: {} at {} ---", log_file, now)?;
    }

    // Ensure the storage/blast directory exists
    let blast_dir = dest.join("storage").join("blast");
    if !blast_dir.exists() {
        fs::create_dir_all(&blast_dir)?;
    }
    
    // Note: We don't create dashboard.kdl here anymore - it should be part of the template
    // The dashboard.kdl file should be present in the cloned template repository
    
    // Ensure the refresh_server_info.sh script is executable
    let script_path = blast_dir.join("refresh_server_info.sh");
    if script_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755); // rwxr-xr-x permissions
            fs::set_permissions(&script_path, perms)?;
        }
    } else {
        // Create the script if it doesn't exist
        let script_content = "#!/bin/bash\n\
                             # Server info refresh script\n\
                             echo \"Refreshing server information...\"\n\
                             # Add server info collection commands here\n";
        let mut file = fs::File::create(&script_path)?;
        file.write_all(script_content.as_bytes())?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755); // rwxr-xr-x permissions
            fs::set_permissions(&script_path, perms)?;
        }
    }

    Ok(())
}

fn update_project(project_path: &Path, project_name: &str) -> std::io::Result<()> {
    // Update Cargo.toml with the project name
    let cargo_toml_path = project_path.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Cargo.toml not found in template"));
    }
    
    let content = fs::read_to_string(&cargo_toml_path)?;
    let mut doc = content.parse::<DocumentMut>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("TOML parse error: {}", e)))?;
    
    // Update package name
    doc["package"]["name"] = value(project_name);
    
    // Write the updated Cargo.toml
    fs::write(cargo_toml_path, doc.to_string())?;
    
    // Update Catalyst.toml if it exists
    let catalyst_toml_path = project_path.join("Catalyst.toml");
    if catalyst_toml_path.exists() {
        let content = fs::read_to_string(&catalyst_toml_path)?;
        let mut doc = content.parse::<DocumentMut>()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("TOML parse error: {}", e)))?;
        
        // Update project name if settings section exists
        if doc.contains_key("settings") {
            doc["settings"]["project_name"] = value(project_name);
        }
        
        // Write the updated Catalyst.toml
        fs::write(catalyst_toml_path, doc.to_string())?;
    }
    
    // Create or update .env file with JWT secret
    let env_path = project_path.join(".env");
    
    // Check if .env already exists
    let env_exists = env_path.exists();
    
    // Create a template .env file if it doesn't exist
    if !env_exists {
        // Check if .env.example exists in the project
        let env_example_path = project_path.join(".env.example");
        
        if env_example_path.exists() {
            // Copy from .env.example
            let env_example = fs::read_to_string(&env_example_path)?;
            fs::write(&env_path, env_example)?;
        } else {
            // Fallback to standard template
            let env_template = "DATABASE_URL=postgres://postgres:postgres@localhost/postgres\n";
            fs::write(&env_path, env_template)?;
        }
    }
    
    // Add JWT secret to .env file
    let mut env_file = fs::OpenOptions::new().append(true).open(&env_path)?;
    writeln!(env_file, "JWT_SECRET={}", generate_jwt_secret())?;
    
    // Prompt user to edit .env file
    if prompt_for_env_edit() {
        edit_env_file(&env_path)?;
    }
    
    // Initialize git repository
    initialize_git_repository(project_path)?;
    
    Ok(())
}

fn prompt_for_env_edit() -> bool {
    use dialoguer::{theme::ColorfulTheme, Confirm};
    use console::style;
    
    // Determine whether to show verbose output based on environment
    let is_verbose = std::env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1";
    
    // Only show database connection details in verbose mode
    if is_verbose {
        // Read the actual DATABASE_URL from .env if it exists, otherwise use a placeholder
        let env_path = std::path::Path::new(".env");
        let db_url = if env_path.exists() {
            match std::fs::read_to_string(env_path) {
                Ok(content) => {
                    content.lines()
                        .find(|line| line.starts_with("DATABASE_URL="))
                        .map(|line| line.trim_start_matches("DATABASE_URL="))
                        .unwrap_or("postgres://postgres:postgres@localhost/postgres")
                        .to_string()
                },
                Err(_) => "postgres://postgres:postgres@localhost/postgres".to_string()
            }
        } else {
            "postgres://postgres:postgres@localhost/postgres".to_string()
        };
        
        println!("\n{} The default database connection is set to:", style("ℹ️").cyan());
        println!("  DATABASE_URL={}", db_url);
        println!();
        println!("This connection uses the public schema by default.");
        println!("For multiple projects, you may want to use different databases or schemas.");
    }

    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to edit the .env file now to customize the database connection?")
        .default(true)
        .interact()
        .unwrap_or(false)
}

fn edit_env_file(env_path: &Path) -> std::io::Result<()> {
    use dialoguer::Editor;
    use console::style;
    
    // Determine whether to show verbose output based on environment
    let is_verbose = std::env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1";

    let current_content = fs::read_to_string(env_path)?;

    // This shows only in verbose mode as it's helpful but not essential information
    if is_verbose {
        println!("\nYou can add multiple database connections as follows:");
        println!("DATABASE_URL=postgres://postgres:postgres@localhost/postgres");
        println!("DATABASE_URL_USERS=postgres://postgres:postgres@localhost/users");
        println!("DATABASE_URL_LOGS=postgres://postgres:postgres@localhost/logs");
        println!("\nThe first connection will be used as the default.");
    }

    // Show what's happening - use standard "editor opening" message
    println!("\n{} Opening .env file in your editor so you can set the values...", 
        style("📝").cyan());
        
    // Get editor from environment
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    println!("{} Using editor: {}", style("ℹ️").cyan(), editor);

    if let Some(edited_content) = Editor::new().edit(&current_content).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Editor error: {}", e)))? {
        fs::write(env_path, edited_content)?;
        
        // Use success icon for success message
        println!("{} All environment variables have been set", style("✅").green());
    } else {
        // No changes made - still confirm
        println!("{} No changes made to .env file", style("ℹ️").cyan());
    }

    Ok(())
}

// Function to initialize a git repository in the project directory
fn initialize_git_repository(project_path: &Path) -> std::io::Result<()> {
    use console::style;
    
    // Get the current directory to return to it later
    let current_dir = std::env::current_dir()?;

    // Change to the project directory
    std::env::set_current_dir(project_path)?;
    
    // Determine whether to show verbose output based on environment
    let is_verbose = std::env::var("BLAST_VERBOSE").unwrap_or_else(|_| String::from("0")) == "1";

    // Initialize git repository - use logger style for main message
    println!("{} Initializing git repository...", style("🔄").cyan());
    
    match Command::new("git").arg("init").output() {
        Ok(output) => {
            if !output.status.success() {
                println!("{} Failed to initialize git repository: {}", 
                    style("❌").red().bold(), 
                    String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => {
            println!("{} Failed to initialize git repository: {}", 
                style("❌").red().bold(), e);
            // Continue with project creation, even if git init fails
        }
    }

    // Create .gitignore file
    // Use current working directory which should now be the project directory
    let gitignore_contents = "\
# Rust artifacts
/target/
**/*.rs.bk
Cargo.lock

# Environment variables
.env

# Logs
/storage/logs/*.log

# IDE files
.idea/
.vscode/
*.iml

# Generated assets
/public/css/
/public/js/
";

    // Write .gitignore in current directory since we've already changed to project directory
    match fs::write(".gitignore", gitignore_contents) {
        Ok(_) => {
            if is_verbose {
                println!("Created .gitignore file");
            }
        },
        Err(e) => println!("{} Failed to create .gitignore file: {}", 
            style("❌").red().bold(), e),
    }

    // Return to the original directory
    std::env::set_current_dir(current_dir)?;

    Ok(())
}
