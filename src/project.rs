use chrono;
use include_dir::{include_dir, Dir};
use rand::Rng;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use toml_edit::{value, DocumentMut};

static TEMPLATE_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/template");

fn generate_jwt_secret() -> String {
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
    println!("Creating new project: {}", project_name);
    let project_path = Path::new(project_name);
    if project_path.exists() {
        eprintln!("Error: Directory {} already exists.", project_name);
        return;
    }
    if let Err(e) = create_and_dump_template(project_path) {
        eprintln!("Failed to create project structure: {}", e);
        return;
    }
    if let Err(e) = update_project(project_path, project_name) {
        eprintln!("Failed to update project configuration: {}", e);
        return;
    }
    println!("Project {} created successfully with initialized git repository.", project_name);
}

fn create_and_dump_template(dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    TEMPLATE_DIR.extract(dest)?;

    // Initialize log files with content to prevent race conditions with zellij
    let logs_dir = dest.join("storage").join("logs");
    if logs_dir.exists() {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_files = ["debug.log", "error.log", "info.log", "server.log", "warning.log"];

        for log_file in log_files.iter() {
            let log_path = logs_dir.join(log_file);
            if log_path.exists() {
                let mut file = fs::OpenOptions::new().write(true).truncate(true).open(&log_path)?;

                writeln!(file, "--- Log initialized: {} at {} ---", log_file, now)?;
            }
        }
    }

    // Ensure the refresh_server_info.sh script is executable
    let blast_dir = dest.join("storage").join("blast");
    let script_path = blast_dir.join("refresh_server_info.sh");
    if script_path.exists() {
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
    let cargo_toml_path = project_path.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Cargo.toml not found in template"));
    }
    let content = fs::read_to_string(&cargo_toml_path)?;
    let mut doc = content.parse::<DocumentMut>().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("TOML parse error: {}", e)))?;
    doc["package"]["name"] = value(project_name);
    fs::write(cargo_toml_path, doc.to_string())?;

    let env_path = project_path.join(".env");
    let mut env_file = fs::OpenOptions::new().append(true).create(true).open(&env_path)?;
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
    
    println!("\nThe default database connection is set to:");
    println!("  DATABASE_URL=postgres://postgres:postgres@localhost/postgres");
    println!("\nThis connection uses the public schema by default.");
    println!("For multiple projects, you may want to use different databases or schemas.");
    
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to edit the .env file now to customize the database connection?")
        .default(true)
        .interact()
        .unwrap_or(false)
}

fn edit_env_file(env_path: &Path) -> std::io::Result<()> {
    use dialoguer::{theme::ColorfulTheme, Editor};
    
    let current_content = fs::read_to_string(env_path)?;
    
    println!("\nYou can add multiple database connections as follows:");
    println!("DATABASE_URL=postgres://postgres:postgres@localhost/postgres");
    println!("DATABASE_URL_USERS=postgres://postgres:postgres@localhost/users");
    println!("DATABASE_URL_LOGS=postgres://postgres:postgres@localhost/logs");
    println!("\nThe first connection will be used as the default.");

    if let Some(edited_content) = Editor::new()
        .edit(&current_content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Editor error: {}", e)))? {
        fs::write(env_path, edited_content)?;
        println!("✓ .env file updated successfully.");
    } else {
        println!("✓ No changes made to .env file.");
    }
    
    Ok(())
}

// Function to initialize a git repository in the project directory
fn initialize_git_repository(project_path: &Path) -> std::io::Result<()> {
    // Get the current directory to return to it later
    let current_dir = std::env::current_dir()?;

    // Change to the project directory
    std::env::set_current_dir(project_path)?;

    // Initialize git repository
    println!("Initializing git repository...");
    match Command::new("git").arg("init").output() {
        Ok(output) => {
            if !output.status.success() {
                println!("Warning: Failed to initialize git repository: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => {
            println!("Warning: Failed to initialize git repository: {}", e);
            // Continue with project creation, even if git init fails
        }
    }

    // Create .gitignore file
    let gitignore_path = project_path.join(".gitignore");
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

    match fs::write(&gitignore_path, gitignore_contents) {
        Ok(_) => println!("Created .gitignore file"),
        Err(e) => println!("Warning: Failed to create .gitignore file: {}", e),
    }

    // Return to the original directory
    std::env::set_current_dir(current_dir)?;

    Ok(())
}
