use crate::commands::Action;
use crate::configs::Config;
use crate::dependencies::DependencyManager;
use console::Style;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use std::error::Error;

// Function to handle actions directly without recursion
pub async fn run_interactive_cli(mut config: Config, dep_manager: &DependencyManager) -> Result<(), Box<dyn Error>> {
    // Set up logging for interactive mode
    use std::fs;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::path::Path;

    // Set the environment variable to indicate we're in interactive mode
    // This ensures any nested command execution knows we're in interactive mode
    std::env::set_var("BLAST_INTERACTIVE", "1");

    // Enable quiet mode to suppress all stdout output
    crate::output::set_quiet_mode(true);

    // Set output mode to log file only - no stdout output
    crate::output::set_output_mode(crate::output::OutputMode::LogFile);

    // Use ANSI clear screen to clean any existing output
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().unwrap(); // Using fully qualified path

    // Set up a single log file for all blast operations
    let project_dir = Path::new(&config.project_dir);

    // Create the storage/blast directory if it doesn't exist
    let blast_dir = project_dir.join("storage").join("blast");
    fs::create_dir_all(&blast_dir)?;

    // Set the log path for the single blast log file
    let blast_log_path = blast_dir.join("blast.log");

    // Open the log in append mode to maintain history
    let mut log_file = OpenOptions::new().create(true).write(true).append(true).open(&blast_log_path)?;

    // Write a session separator to the log
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(log_file, "\n\n=== New Interactive Session: {} ===", now)?;
    writeln!(log_file, "Project: {}", config.project_name)?;
    writeln!(log_file, "Environment: {}", config.environment)?;
    writeln!(log_file, "=======================================")?;
    writeln!(log_file, "Waiting for command selection...")?;

    // Store the log path for later use in the output system
    crate::output::set_log_file_path(&blast_log_path)?;

    // Since we're now using a single log file, set both paths to the same file
    // This ensures backward compatibility with existing code
    crate::output::set_operations_log_path(&blast_log_path);
    crate::output::set_progress_log_path(&blast_log_path);

    let commands = vec![
        // APP commands first (most important)
        "[APP] Refresh",
        "[APP] Run Server", 
        "[APP] Launch Dashboard",
        "[APP] Toggle Dev/Prod",
        // Code generation group
        "[CODEGEN] Schema",
        "[CODEGEN] Structs",
        "[CODEGEN] Models",
        // DB commands
        "[DB] New Migration",
        "[DB] Migrate",
        "[DB] Rollback",
        // Assets management
        "[Locale] Edit Key",
        "[Assets] Transpile SCSS",
        "[Assets] Minify CSS",
        "[Assets] Publish JS",
        "[Assets] Download CDN",
        // Git commands
        "[GIT] Manager",
        "[GIT] Status",
        "[GIT] Pull",
        "[GIT] Push",
        "[GIT] Commit",
        // Exit is always last
        "[Exit] Kill Session",
    ];

    // Initialize console styles
    let prod_style = Style::new().bold().fg(console::Color::Green);
    let dev_style = Style::new().bold().fg(console::Color::Yellow);

    loop {
        // Clear screen before showing menu to prevent history accumulation
        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush().unwrap();

        let prompt = if config.environment == "prod" {
            format!("{}->[{}] ", prod_style.apply_to(format!("[🚀{}]", config.environment.to_uppercase())), config.project_name)
        } else {
            format!("{}->[{}] ", dev_style.apply_to(format!("[🔧{}]", config.environment.to_uppercase())), config.project_name)
        };

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default()).with_prompt(prompt).items(&commands).default(0).interact().unwrap();

        // Create a single function to log messages to our blast log
        let log_message = |message: &str| -> std::io::Result<()> {
            let mut file = OpenOptions::new().create(true).append(true).open(&blast_log_path)?;

            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            writeln!(file, "[{}] {}", now, message)?;
            Ok(())
        };

        // For API compatibility with the existing code, create aliases that both use the same function
        let update_progress = |message: &str| -> std::io::Result<()> { log_message(message) };

        let log_operation = |message: &str| -> std::io::Result<()> { log_message(message) };

        // Write a start message to the log
        let _ = log_message("Interactive CLI session started");

        // Get a string representation of the command
        let command_name = commands[selection];

        // Update the progress log with what we're about to do
        let _ = update_progress(&format!("Starting operation: {}", command_name));

        // Log the start of the operation
        let _ = log_operation(&format!("Starting operation: {}", command_name));

        // Make sure quiet mode is enabled for this action
        crate::output::set_quiet_mode(true);

        let action = match commands[selection] {
            // APP commands
            "[APP] Refresh" => Action::RefreshApp,
            "[APP] Run Server" => {
                // Set environment variable based on Config
                let show_warnings = config.show_compiler_warnings.to_string();
                std::env::set_var("BLAST_SHOW_WARNINGS", show_warnings);
                
                if config.environment == "prod" || config.environment == "production" {
                    Action::RunProdServer
                } else {
                    Action::RunDevServer
                }
            }
            "[APP] Launch Dashboard" => Action::LaunchDashboard,
            "[APP] Toggle Dev/Prod" => Action::ToggleEnvironment,

            // Codegen commands
            "[CODEGEN] Schema" => Action::GenerateSchema,
            "[CODEGEN] Structs" => Action::GenerateStructs,
            "[CODEGEN] Models" => Action::GenerateModels,

            // DB commands
            "[DB] New Migration" => Action::NewMigration,
            "[DB] Migrate" => Action::Migrate,
            "[DB] Rollback" => Action::Rollback,

            // Asset management
            "[Locale] Edit Key" => Action::EditLocaleKey,
            "[Assets] Transpile SCSS" => Action::TranspileScss,
            "[Assets] Minify CSS" => Action::MinifyCss,
            "[Assets] Publish JS" => Action::ProcessJs,
            "[Assets] Download CDN" => Action::DownloadCdn,
            
            // Git commands
            "[GIT] Manager" => Action::GitManager,
            "[GIT] Status" => Action::GitStatus,
            "[GIT] Pull" => Action::GitPull,
            "[GIT] Push" => Action::GitPush,
            "[GIT] Commit" => Action::GitCommit,

            // Exit
            "[Exit] Kill Session" => Action::Exit,
            _ => continue,
        };

        if action == Action::Exit {
            // Log the exit
            let _ = log_operation("Killing Zellij session...");
            let _ = update_progress("Killing Zellij session...");
            
            // Use zellij to exit the session
            let _ = std::process::Command::new("zellij")
                .args(["kill-session"])
                .spawn();
                
            // If that doesn't work, kill all sessions
            let _ = std::process::Command::new("zellij")
                .args(["kill-all-sessions", "-y"])
                .spawn();
                
            // Break the loop - though we might not get here if zellij properly terminates
            break;
        } else {
            // Clear screen before executing command to clean up any lingering output
            print!("\x1B[2J\x1B[1;1H");
            std::io::stdout().flush().unwrap();

            // Execute the action but capture stdout to prevent output
            let _ = update_progress(&format!("Executing: {}", command_name));

            // We're using our quiet mode system to suppress output
            // All commands executed from here will respect the quiet mode flag
            let result = match handle_action(action, &mut config, dep_manager).await {
                Ok(_) => {
                    // Log the successful completion
                    let success_msg = format!("Operation completed successfully: {}", command_name);
                    let _ = log_operation(&success_msg);
                    let _ = update_progress(&format!("✅ {}", success_msg));

                    // Clear the terminal screen to prevent command history from accumulating
                    print!("\x1B[2J\x1B[1;1H");
                    std::io::stdout().flush().unwrap();
                    true
                }
                Err(e) => {
                    // Log the error
                    let error_msg = format!("Operation failed: {}", e);
                    let _ = log_operation(&error_msg);
                    let _ = update_progress(&format!("❌ {}", error_msg));

                    // Clear the terminal screen to prevent command history from accumulating
                    print!("\x1B[2J\x1B[1;1H");
                    std::io::stdout().flush().unwrap();
                    false
                }
            };

            // No restoration needed - our quiet mode flag handles suppression
            // of stdout/stderr at a higher level

            if !result {
                // Sleep briefly to make sure the error message is seen
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }

    Ok(())
}

// Function to handle actions directly without going through execute_action
async fn handle_action(action: Action, config: &mut Config, dep_manager: &DependencyManager) -> Result<(), Box<dyn Error>> {
    // Helper to log to our single blast log
    let log_progress = |message: &str| -> std::io::Result<()> {
        // Use the output system's log function, which will write to the single blast.log file
        crate::output::log(message)?;
        Ok(())
    };

    match action {
        Action::GitManager => {
            log_progress("Launching Git manager")?;
            crate::git::launch_manager();
            Ok(())
        }
        Action::GitStatus => {
            log_progress("Showing Git repository status")?;
            crate::git::git_status();
            Ok(())
        }
        Action::GitPull => {
            log_progress("Pulling from remote repository")?;
            crate::git::git_pull();
            Ok(())
        }
        Action::GitPush => {
            log_progress("Pushing to remote repository")?;
            crate::git::git_push();
            Ok(())
        }
        Action::GitCommit => {
            log_progress("Committing changes")?;
            crate::git::git_commit();
            Ok(())
        }
        Action::CatalystManager => {
            log_progress("Launching Catalyst.toml configuration manager")?;
            crate::configs::launch_manager(config);
            Ok(())
        },
        Action::NewMigration => {
            dep_manager.ensure_installed(&["diesel"], true)?;
            log_progress("Creating new migration")?;
            crate::database::new_migration();
            Ok(())
        }
        Action::Migrate => {
            dep_manager.ensure_installed(&["diesel"], true)?;
            let success = crate::database::migrate();
            if !success {
                log_progress("Warning: Some migration issues occurred")?;
            } else {
                log_progress("Migrations completed successfully")?;
            }
            Ok(())
        }
        Action::Rollback => {
            dep_manager.ensure_installed(&["diesel"], true)?;
            log_progress("Rolling back migrations")?;
            crate::database::rollback_all();
            Ok(())
        }
        Action::GenerateSchema => {
            dep_manager.ensure_installed(&["diesel"], true)?;
            let success = crate::database::generate_schema();
            if !success {
                log_progress("Warning: Some schema generation issues occurred")?;
            } else {
                log_progress("Schema generated successfully")?;
            }
            Ok(())
        }
        Action::GenerateStructs => {
            let success = crate::structs::generate(config);
            if !success {
                log_progress("Warning: Some struct generation issues occurred")?;
            } else {
                log_progress("Structs generated successfully")?;
            }
            Ok(())
        }
        Action::GenerateModels => {
            let success = crate::models::generate(config);
            if !success {
                log_progress("Warning: Some model generation issues occurred")?;
            } else {
                log_progress("Models generated successfully")?;
            }
            Ok(())
        }
        Action::EditLocaleKey => {
            log_progress("Editing locale key")?;
            crate::locale::edit_key();
            Ok(())
        }
        Action::RefreshApp => {
            // Database operations
            dep_manager.ensure_installed(&["diesel"], true)?;
            log_progress("🔧 Rolling back migrations...")?;
            let rollback_ok = crate::database::rollback_all();
            log_progress("🔧 Running migrations...")?;
            let migrations_ok = crate::database::migrate();
            log_progress("🔧 Seeding database...")?;
            let seed_ok = crate::database::seed(Some(0));
            
            // Code generation operations
            log_progress("🔧 Running code generation...")?;
            log_progress("  → Generating schema...")?;
            let schema_ok = crate::database::generate_schema();
            log_progress("  → Generating structs...")?;
            let structs_ok = crate::structs::generate(config);
            log_progress("  → Generating models...")?;
            let models_ok = crate::models::generate(config);
            
            // Asset operations
            log_progress("🔧 Processing frontend assets...")?;
            log_progress("  → Downloading CDN assets...")?;
            let cdn_ok = match crate::assets::download_assets_async(config).await {
                Ok(_) => true,
                Err(_) => false
            };
            log_progress("  → Transpiling SCSS...")?;
            let scss_ok = match crate::assets::transpile_all_scss(config).await {
                Ok(_) => true,
                Err(_) => false
            };
            log_progress("  → Processing CSS...")?;
            let css_ok = match crate::assets::minify_css_files(config).await {
                Ok(_) => true,
                Err(_) => false
            };
            log_progress("  → Processing JS...")?;
            let js_ok = match crate::assets::process_js(config).await {
                Ok(_) => true,
                Err(_) => false
            };
            
            // Report overall success
            if rollback_ok && migrations_ok && seed_ok && schema_ok && structs_ok && 
               models_ok && cdn_ok && scss_ok && css_ok && js_ok {
                log_progress("\x1b[32m✔\x1b[0m App refresh complete!")?;
            } else {
                log_progress("\x1b[32m✔\x1b[0m App refresh completed with some warnings")?;
            }
            Ok(())
        }
        Action::TranspileScss => {
            dep_manager.ensure_installed(&["sass"], true)?;
            log_progress("Transpiling SCSS files")?;
            crate::assets::transpile_all_scss(config).await
        }
        Action::MinifyCss => {
            log_progress("Minifying CSS files")?;
            crate::assets::minify_css_files(config).await
        }
        Action::ProcessJs => {
            log_progress("Processing JS files")?;
            crate::assets::process_js(config).await
        }
        Action::DownloadCdn => {
            log_progress("Downloading CDN assets")?;
            crate::assets::download_assets_async(config).await
        }
        Action::RunDevServer => {
            // Use the setting from Config
            let show_warnings = config.show_compiler_warnings;
            
            if let Ok(pid) = crate::dashboard::start_server(config, true) {
                let warnings_mode = if show_warnings { "with" } else { "without" };
                log_progress(&format!("Development server started {} warnings (PID: {})", warnings_mode, pid))?;
            } else {
                // Set up proper environment and flags to control warnings
                let (cargo_env, cargo_flags) = if show_warnings {
                    ("", "")
                } else {
                    ("RUSTFLAGS=\"-Awarnings\" ", "--quiet ")
                };
                
                std::process::Command::new("script")
                    .args(["-q", "-c", &format!("{} cargo run {}--bin {}", cargo_env, cargo_flags, &config.project_name), "storage/logs/server.log"])
                    .spawn()?;
                
                let warnings_mode = if show_warnings { "with" } else { "without" };
                log_progress(&format!("Development server started {} warnings using cargo run", warnings_mode))?;
            }
            Ok(())
        }
        Action::RunProdServer => {
            // Use the setting from Config
            let show_warnings = config.show_compiler_warnings;
            
            if let Ok(pid) = crate::dashboard::start_server(config, false) {
                let warnings_mode = if show_warnings { "with" } else { "without" };
                log_progress(&format!("Production server started {} warnings (PID: {})", warnings_mode, pid))?;
            } else {
                // Check if the binary exists in the target/release directory
                let binary_path = format!("target/release/{}", &config.project_name);
                if std::path::Path::new(&binary_path).exists() {
                    // Use the compiled binary
                    std::process::Command::new("script").args(["-q", "-c", &binary_path, "storage/logs/server.log"]).spawn()?;
                    log_progress(&format!("Production server started using compiled binary: {}", binary_path))?;
                } else {
                    // Fallback to cargo run --release
                    // Set up proper environment and flags to control warnings
                    let (cargo_env, cargo_flags) = if show_warnings {
                        ("", "")
                    } else {
                        ("RUSTFLAGS=\"-Awarnings\" ", "--quiet ")
                    };
                    
                    std::process::Command::new("script")
                        .args(["-q", "-c", &format!("{} cargo run {}--release --bin {}", cargo_env, cargo_flags, &config.project_name), "storage/logs/server.log"])
                        .spawn()?;
                    
                    let warnings_mode = if show_warnings { "with" } else { "without" };
                    log_progress(&format!("Production server started {} warnings using cargo run --release", warnings_mode))?;
                    log_progress("Tip: Build with 'cargo build --release' for faster startup next time")?;
                }
            }
            Ok(())
        }
        Action::LaunchDashboard => {
            // Silently ensure required dependencies are installed
            dep_manager.ensure_installed(&["zellij", "diesel"], false)?;
            log_progress("Launching dashboard")?;
            crate::dashboard::launch_dashboard(config)?;
            Ok(())
        }
        Action::ToggleEnvironment => {
            // Toggle environment
            let old_env = config.environment.clone();
            crate::configs::toggle_environment(config)?;
            // Log confirmation message
            let env_msg = if config.environment == "prod" || config.environment == "production" {
                format!("Switched from {} to production mode", old_env)
            } else {
                format!("Switched from {} to development mode", old_env)
            };
            log_progress(&format!("\x1b[32m✔\x1b[0m Environment toggled: {}", env_msg))?;
            log_progress("Run `blast scss`, `blast css`, or `blast js` to rebuild assets with new settings")?;
            Ok(())
        }
        _ => Ok(()),
    }
}
