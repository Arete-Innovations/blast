use crate::error::{BlastError, BlastResult};
use chrono;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use toml_edit::{value, DocumentMut};

const TEMPLATE_REPOS: [&str; 3] = [
    "https://github.com/Arete-Innovations/catalyst.git",
    "https://gitlab.com/Arete-Innovations/catalyst.git",
    "https://bitbucket.org/Arete-Innovations/catalyst.git",
];

const CLONE_TIMEOUT: Duration = Duration::from_secs(30);

fn verbose_flag() -> bool {
    match std::env::var("BLAST_VERBOSE") {
        Ok(v) => v == "1",
        Err(_e) => {
            false
        }
    }
}

pub fn create_new_project(project_name: &str, use_dev_branch: bool) {
    use console::style;

    if use_dev_branch {
        println!("{} project: {} using {} branch", style("Creating new").green().bold(), style(project_name).cyan(), style("dev").yellow());
    } else {
        println!("{} project: {}", style("Creating new").green().bold(), style(project_name).cyan());
    }
    let project_path = Path::new(project_name);

    if project_path.exists() {
        eprintln!("{} Directory {} already exists.", style("Error:").red().bold(), project_name);
        return;
    }

    let temp_dir = format!("{}_temp", project_name);
    let temp_path = Path::new(&temp_dir);

    if temp_path.exists() {
        println!("{} Cleaning up temporary directory...", style("⚙").cyan());
        if let Err(e) = fs::remove_dir_all(temp_path) {
            eprintln!("{} Failed to clean up temporary directory: {}", style("Error:").red().bold(), e);
            return;
        }
    }

    println!("{} Fetching project template...", style("📥").cyan());
    if let Err(e) = create_and_dump_template(temp_path, use_dev_branch) {
        eprintln!("{} Failed to create project structure: {}", style("Error:").red().bold(), e);

        if temp_path.exists() {
            if let Err(cleanup_err) = fs::remove_dir_all(temp_path) {
                eprintln!("cleanup failed: {}", cleanup_err);
            }
        }

        return;
    }

    println!("{} Creating project directory...", style("📂").cyan());
    if let Err(e) = fs::rename(temp_path, project_path) {
        eprintln!("{} Failed to create project directory: {}", style("Error:").red().bold(), e);

        if temp_path.exists() {
            if let Err(cleanup_err) = fs::remove_dir_all(temp_path) {
                eprintln!("cleanup failed: {}", cleanup_err);
            }
        }

        return;
    }

    println!("{} Configuring project...", style("⚙").cyan());
    if let Err(e) = update_project(project_path, project_name) {
        eprintln!("{} Failed to update project configuration: {}", style("Error:").red().bold(), e);
        return;
    }

    println!(
        "\n{} Project {} created successfully! {}",
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

fn create_and_dump_template(dest: &Path, use_dev_branch: bool) -> BlastResult<()> {
    fs::create_dir_all(dest)?;

    let mut clone_successful = false;
    let mut last_error = String::new();

    let is_verbose = verbose_flag();

    let branch = if use_dev_branch { "dev" } else { "master" };

    for repo_url in TEMPLATE_REPOS.iter() {
        if is_verbose {
            println!("Attempting to clone template from: {}", repo_url);
        }

        let mut cmd = Command::new("git");
        cmd.args([
            "clone",
            "--depth=1",
            "--single-branch",
            "--branch",
            branch,
            "--config",
            &format!("core.askPass=echo"),
            "--config",
            &format!("http.connectTimeout={}", CLONE_TIMEOUT.as_secs()),
            "--config",
            &format!("http.lowSpeedLimit=1000"),
            "--config",
            &format!("http.lowSpeedTime={}", CLONE_TIMEOUT.as_secs()),
            repo_url,
            &dest.to_string_lossy(),
        ]);

        if !is_verbose {
            if cfg!(target_os = "windows") {
                cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            } else {
                cmd.arg("--quiet");
            }
        }

        let status = cmd.status();

        match status {
            Ok(exit_status) if exit_status.success() => {
                clone_successful = true;

                if is_verbose {
                    println!("Successfully cloned template repository.");
                }

                let git_dir = dest.join(".git");
                if git_dir.exists() {
                    fs::remove_dir_all(git_dir)?;
                }

                break;
            }
            Ok(exit_status) => {
                last_error = format!("Git clone command failed for repository {} (exit {})", repo_url, exit_status);
            }
            Err(e) => {
                last_error = format!("Failed to execute git clone: {}", e);
            }
        }
    }

    if !clone_successful {
        return Err(BlastError::Project(format!(
            "Failed to clone template from any repository. Last error: {}",
            last_error
        )));
    }

    let logs_dir = dest.join("storage").join("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(&logs_dir)?;
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_files = ["debug.log", "error.log", "info.log", "server.log", "warning.log"];

    for log_file in log_files.iter() {
        let log_path = logs_dir.join(log_file);
        let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).open(&log_path)?;

        writeln!(file, "--- Log initialized: {} at {} ---", log_file, now)?;
    }

    let blast_dir = dest.join("storage").join("blast");
    if !blast_dir.exists() {
        fs::create_dir_all(&blast_dir)?;
    }

    let script_path = blast_dir.join("refresh_server_info.sh");
    if script_path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }
    } else {
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
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }
    }

    Ok(())
}

fn update_project(project_path: &Path, project_name: &str) -> BlastResult<()> {
    let cargo_toml_path = project_path.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err(BlastError::NotFound("Cargo.toml not found in template".to_string()));
    }

    let content = fs::read_to_string(&cargo_toml_path)?;
    let mut doc = content
        .parse::<DocumentMut>()
        .map_err(|e| BlastError::Project(format!("TOML parse error: {}", e)))?;

    doc["package"]["name"] = value(project_name);

    fs::write(cargo_toml_path, doc.to_string())?;

    let env_path = project_path.join(".env");

    if !env_path.exists() {
        let env_example_path = project_path.join(".env.example");

        if env_example_path.exists() {
            let env_example = fs::read_to_string(&env_example_path)?;
            fs::write(&env_path, env_example)?;
        } else {
            let env_template = "DATABASE_URL=postgres://postgres:postgres@localhost/postgres\n";
            fs::write(&env_path, env_template)?;
        }
    }

    if prompt_for_env_edit() {
        edit_env_file(&env_path)?;
    }

    initialize_git_repository(project_path)?;

    Ok(())
}

fn prompt_for_env_edit() -> bool {
    use console::style;
    use dialoguer::{theme::ColorfulTheme, Confirm};

    let is_verbose = verbose_flag();

    if is_verbose {
        let env_path = std::path::Path::new(".env");
        let default_url = "postgres://postgres:postgres@localhost/postgres".to_string();
        let db_url = if env_path.exists() {
            match std::fs::read_to_string(env_path) {
                Ok(content) => {
                    let line_match = content
                        .lines()
                        .find(|line| line.starts_with("DATABASE_URL="))
                        .map(|line| line.trim_start_matches("DATABASE_URL=").to_string());
                    match line_match {
                        Some(url) => url,
                        None => default_url.clone(),
                    }
                }
                Err(_e) => default_url.clone(),
            }
        } else {
            default_url.clone()
        };

        println!("\n{} The default database connection is set to:", style("ℹ️").cyan());
        println!("  DATABASE_URL={}", db_url);
        println!();
        println!("This connection uses the public schema by default.");
        println!("For multiple projects, you may want to use different databases or schemas.");
    }

    match Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to edit the .env file now to customize the database connection?")
        .default(true)
        .interact()
    {
        Ok(answer) => answer,
        Err(_e) => {
            false
        }
    }
}

fn edit_env_file(env_path: &Path) -> BlastResult<()> {
    use console::style;
    use dialoguer::Editor;

    let is_verbose = verbose_flag();

    let current_content = fs::read_to_string(env_path)?;

    if is_verbose {
        println!("\nYou can add multiple database connections as follows:");
        println!("DATABASE_URL=postgres://postgres:postgres@localhost/postgres");
        println!("DATABASE_URL_USERS=postgres://postgres:postgres@localhost/users");
        println!("DATABASE_URL_LOGS=postgres://postgres:postgres@localhost/logs");
        println!("\nThe first connection will be used as the default.");
    }

    println!("\n{} Opening .env file in your editor so you can set the values...", style("📝").cyan());

    let editor = match std::env::var("EDITOR") {
        Ok(v) => v,
        Err(_e) => "nano".to_string(),
    };
    println!("{} Using editor: {}", style("ℹ️").cyan(), editor);

    let edited = Editor::new()
        .executable(editor)
        .edit(&current_content)
        .map_err(|e| BlastError::Project(format!("Editor error: {}", e)))?;

    match edited {
        Some(edited_content) => {
            fs::write(env_path, edited_content)?;
            println!("{} All environment variables have been set", style("✅").green());
        }
        None => {
            println!("{} No changes made to .env file", style("ℹ️").cyan());
        }
    }

    Ok(())
}

fn initialize_git_repository(project_path: &Path) -> BlastResult<()> {
    use console::style;

    let current_dir = std::env::current_dir()?;

    std::env::set_current_dir(project_path)?;

    let is_verbose = verbose_flag();

    println!("{} Initializing git repository...", style("🔄").cyan());

    match Command::new("git").arg("init").output() {
        Ok(output) => {
            if !output.status.success() {
                println!("{} Failed to initialize git repository: {}", style("❌").red().bold(), String::from_utf8_lossy(&output.stderr));
            }
        }
        Err(e) => {
            println!("{} Failed to initialize git repository: {}", style("❌").red().bold(), e);
        }
    }

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

    match fs::write(".gitignore", gitignore_contents) {
        Ok(_v) => {
            if is_verbose {
                println!("Created .gitignore file");
            }
        }
        Err(e) => println!("{} Failed to create .gitignore file: {}", style("❌").red().bold(), e),
    }

    std::env::set_current_dir(current_dir)?;

    Ok(())
}
