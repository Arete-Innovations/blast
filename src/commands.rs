use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::progress;
use crate::{assets, dashboard, database, locale, models, project, structs};
use std::error::Error;
use std::process::Command;

// Function to run post-generation hooks defined in Catalyst.toml
fn run_post_generation_hooks(config: &Config, hook_type: &str) -> bool {
    // Check if hooks are enabled
    let hooks_enabled = config.assets.get("codegen").and_then(|c| c.get("hooks")).and_then(|h| h.get("enabled")).and_then(|e| e.as_bool()).unwrap_or(false);

    if !hooks_enabled {
        return true; // Hooks disabled, return success
    }

    // Get the hooks for the specific type
    let specific_hooks: Vec<String> = config
        .assets
        .get("codegen")
        .and_then(|c| c.get("hooks"))
        .and_then(|h| h.get(&format!("post_{}", hook_type)))
        .and_then(|h| h.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Get the hooks for any generation
    let any_hooks: Vec<String> = config
        .assets
        .get("codegen")
        .and_then(|c| c.get("hooks"))
        .and_then(|h| h.get("post_any"))
        .and_then(|h| h.as_array())
        .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Combine both hook lists
    let hooks: Vec<String> = [specific_hooks, any_hooks].concat();

    if hooks.is_empty() {
        return true; // No hooks defined, return success
    }

    println!("Running post-generation hooks for {}", hook_type);

    let mut success = true;

    for hook in hooks {
        let progress = progress::ProgressManager::new_spinner();
        progress.set_message(&format!("Running hook: {}", hook));

        // Split the command string into program and arguments
        let parts: Vec<&str> = hook.split_whitespace().collect();
        if parts.is_empty() {
            progress.error("Empty hook command");
            success = false;
            continue;
        }

        let program = parts[0];
        let args = &parts[1..];

        // Run the command
        let result = Command::new(program).args(args).output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    progress.success(&format!("Hook executed successfully: {}", hook));

                    // Print command output if non-empty
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !stdout.trim().is_empty() {
                        println!("Output: {}", stdout);
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    progress.error(&format!("Hook failed: {} - {}", hook, stderr));
                    success = false;
                }
            }
            Err(e) => {
                progress.error(&format!("Failed to execute hook: {} - {}", hook, e));
                success = false;
            }
        }
    }

    success
}

#[derive(PartialEq, Clone)]
pub enum Action {
    NewProject(String),
    NewMigration,
    Migrate,
    Rollback,
    Seed(Option<String>), // Run database seed with optional specific file
    GenerateSchema,
    GenerateStructs,
    GenerateModels,
    EditLocaleKey,
    LocaleManager,
    RefreshApp,
    TranspileScss,
    MinifyCss,
    ProcessJs,
    PublishCss,
    DownloadCdn,
    RunDevServer,
    RunProdServer,
    LaunchDashboard,
    ToggleEnvironment,
    RunInteractiveCli,
    GitManager,
    GitStatus,
    GitPull,
    GitPush,
    GitCommit,
    CatalystManager,
    CargoAdd(String), // Add a new dependency with optional search term
    CargoRemove,      // Remove a dependency
    LogTruncate(Option<String>), // Truncate logs (all or specific file)
    Exit,
    Help,
}

// Define mapping between CLI commands and actions - with one-shot commands for all interactive features
pub fn parse_cli_args(args: &[String]) -> Option<Action> {
    match args.get(1).map(|s| s.as_str()) {
        // Project creation
        Some("new") if args.len() >= 3 => Some(Action::NewProject(args[2].clone())),

        // App commands
        Some("refresh") => Some(Action::RefreshApp),
        Some("run") => Some(Action::RunDevServer),
        Some("run-prod") => Some(Action::RunProdServer),
        Some("dashboard") => Some(Action::LaunchDashboard),
        Some("toggle-env") => Some(Action::ToggleEnvironment),

        // DB commands
        Some("migration") => Some(Action::NewMigration),
        Some("migrate") => Some(Action::Migrate),
        Some("rollback") => Some(Action::Rollback),
        Some("seed") => {
            if args.len() >= 3 {
                Some(Action::Seed(Some(args[2].clone())))
            } else {
                Some(Action::Seed(None))
            }
        },
        Some("schema") => Some(Action::GenerateSchema),

        // Asset management
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("structs") => Some(Action::GenerateStructs),
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("models") => Some(Action::GenerateModels),
        Some("locale") => Some(Action::EditLocaleKey),
        Some("locale-manager") => Some(Action::LocaleManager),
        Some("scss") => Some(Action::TranspileScss),
        Some("css") => Some(Action::MinifyCss),
        Some("publish-css") => Some(Action::PublishCss),
        Some("js") => Some(Action::ProcessJs),
        Some("cdn") => Some(Action::DownloadCdn),

        // Help and interactive mode
        Some("help") | Some("-h") | Some("--help") => Some(Action::Help),
        Some("cli") => Some(Action::RunInteractiveCli), // Interactive CLI mode

        // Git commands
        Some("git") if args.len() < 3 => Some(Action::GitManager),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("status") => Some(Action::GitStatus),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("pull") => Some(Action::GitPull),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("push") => Some(Action::GitPush),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("commit") => Some(Action::GitCommit),

        // Catalyst manager
        Some("catalyst") => Some(Action::CatalystManager),

        // Cargo commands
        Some("cargo") if args.get(2).map(|s| s.as_str()) == Some("add") => {
            let search_term = args.get(3).map(|s| s.clone()).unwrap_or_default();
            Some(Action::CargoAdd(search_term))
        }
        Some("cargo") if args.get(2).map(|s| s.as_str()) == Some("remove") => Some(Action::CargoRemove),

        // Log management
        Some("logs") | Some("log") if args.get(2).map(|s| s.as_str()) == Some("truncate") => {
            if args.len() >= 4 {
                Some(Action::LogTruncate(Some(args[3].clone())))
            } else {
                Some(Action::LogTruncate(None))
            }
        },

        // Legacy command handling
        Some("serve") => Some(Action::RunDevServer),
        Some("serve-prod") => Some(Action::RunProdServer),
        Some("env") => Some(Action::ToggleEnvironment),

        _ => None,
    }
}

// Show help information
pub fn show_help() {
    println!("Blast - Suckless Web Framework CLI");
    println!();
    println!("USAGE:");
    println!("  blast [COMMAND]");
    println!();
    println!("APP COMMANDS:");
    println!("  refresh              Refresh the application (rollback, migrate, seed, gen schema & structs)");
    println!("  run                  Run the development server");
    println!("  run-prod             Run the production server");
    println!("  dashboard            Launch the interactive dashboard");
    println!("  toggle-env           Toggle between development and production environments");
    println!();
    println!("DATABASE COMMANDS:");
    println!("  migration            Create a new migration");
    println!("  migrate              Run all pending migrations");
    println!("  rollback             Rollback all migrations");
    println!("  seed [file]          Run database seeds (all or specific file)");
    println!("  schema               Generate database schema");
    println!();
    println!("ASSET MANAGEMENT:");
    println!("  gen structs          Generate structs from schema");
    println!("  gen models           Generate model implementations");
    println!("  locale               Edit locale keys");
    println!("  locale-manager       Launch interactive locale management interface");
    println!("  scss                 Transpile SCSS files");
    println!("  css                  Minify CSS files");
    println!("  publish-css          Copy CSS files from src/assets/css to public/css with optional minification");
    println!("  js                   Process JS files");
    println!("  cdn                  Download CDN assets");
    println!();
    println!("CARGO COMMANDS:");
    println!("  cargo add [search]    Add a dependency - search crates.io if term provided");
    println!("  cargo remove          Remove a dependency interactively");
    println!();
    println!("LOG MANAGEMENT:");
    println!("  log truncate [file]   Truncate log files (all or specific file)");
    println!();
    println!("GIT COMMANDS:");
    println!("  git                  Launch interactive Git manager");
    println!("  git status           Show Git repository status");
    println!("  git pull             Pull from remote repository");
    println!("  git push             Push to remote repository");
    println!("  git commit           Commit changes with a message");
    println!();
    println!("CONFIG COMMANDS:");
    println!("  catalyst             Launch interactive Catalyst.toml configuration manager");
    println!();
    println!("OTHER COMMANDS:");
    println!("  new <project_name>   Create a new project");
    println!("  cli                  Run interactive CLI menu (for use in dashboard panes)");
    println!("  help                 Show this help message");
    println!();
    println!("NOTES:");
    println!("  - Running 'blast' without arguments launches the interactive dashboard");
    println!("  - Every interactive command has a one-shot equivalent");
    println!("  - For server commands, 'run' will use --bin in dev mode and the binary in prod mode");
}

// Execute an action with the provided config
pub async fn execute_action(action: Action, config: Option<&mut Config>, dep_manager: &DependencyManager) -> Result<(), Box<dyn Error>> {
    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    // If in interactive mode, we'll use a completely different approach to logging
    if is_interactive {
        // All output should go to log files, not stdout
        crate::output::set_quiet_mode(true);
    }

    // Get the configuration to use for this action
    let mut current_config = if let Some(cfg) = config {
        // If a config was provided, reload it from disk to ensure it's fresh
        let mut cfg_clone = cfg.clone();

        // Try to reload the config, but don't fail if it doesn't work
        let _ = crate::configs::reload_config(&mut cfg_clone);

        cfg_clone
    } else {
        // No config provided, load a fresh one or use defaults
        match crate::configs::get_project_info() {
            Ok(cfg) => cfg,
            Err(_) => {
                // Use default config for actions that don't require it
                Config {
                    environment: "dev".to_string(),
                    project_name: "unknown".to_string(),
                    assets: toml::Value::Table(toml::value::Table::new()),
                    project_dir: std::env::current_dir().unwrap_or_default(),
                    show_compiler_warnings: true,
                }
            }
        }
    };

    match action {
        Action::RunInteractiveCli => {
            // Run the interactive CLI directly without going through execute_action
            crate::interactive::run_interactive_cli(current_config, dep_manager).await
        }
        Action::GitManager => {
            println!("Launching Git manager...");
            crate::git::launch_manager();
            Ok(())
        }
        Action::GitStatus => {
            println!("Git repository status:");
            crate::git::git_status();
            Ok(())
        }
        Action::GitPull => {
            println!("Pulling from remote repository...");
            crate::git::git_pull();
            Ok(())
        }
        Action::GitPush => {
            println!("Pushing to remote repository...");
            crate::git::git_push();
            Ok(())
        }
        Action::GitCommit => {
            println!("Committing changes...");
            crate::git::git_commit();
            Ok(())
        }
        Action::CatalystManager => {
            println!("Launching Catalyst.toml configuration manager...");
            crate::configs::launch_manager(&mut current_config);
            Ok(())
        }
        Action::NewProject(name) => {
            // No global progress bar, just run each step with its own progress updates

            println!("🔧 Creating project structure...");
            project::create_new_project(&name);

            let cwd = std::env::current_dir()?;
            let project_dir = format!("{}/{}", cwd.display(), name);
            let config_path = format!("{}/Catalyst.toml", project_dir);
            let project_config = crate::configs::get_project_info_with_paths(&config_path, &project_dir)?;

            println!("🔧 Downloading CDN assets...");
            assets::download_assets_async(&project_config).await?;

            println!("🔧 Processing frontend assets...");
            assets::process_all_assets(&project_config).await?;

            // Change to the project directory to run migrations
            std::env::set_current_dir(&project_dir)?;

            // Ensure diesel is installed
            dep_manager.ensure_installed(&["diesel"], true)?;

            println!("🔧 Setting up database structure...");
            // Run migrations
            let migrations_ok = database::migrate();

            println!("🔧 Setting up seed data and schema...");
            // Run seeds (assuming default seed level 0)
            let seeds_ok = database::seed(Some(0));

            // Generate schema
            let schema_ok = database::generate_schema();

            // Determine overall success
            if migrations_ok && seeds_ok && schema_ok {
                println!("\x1b[32m✔\x1b[0m Project setup complete!");
            } else {
                println!("\x1b[32m✔\x1b[0m Project setup completed with some warnings");
            }
            Ok(())
        }
        Action::NewMigration => {
            // Ensure diesel is installed
            dep_manager.ensure_installed(&["diesel"], true)?;

            database::new_migration();
            Ok(())
        }
        Action::Migrate => {
            // Ensure diesel is installed
            dep_manager.ensure_installed(&["diesel"], true)?;

            let success = database::migrate();
            if !success {
                println!("Warning: Some migration issues occurred");
            }
            Ok(())
        }
        Action::Rollback => {
            // Ensure diesel is installed
            dep_manager.ensure_installed(&["diesel"], true)?;

            let success = database::rollback_all();
            if !success {
                println!("Warning: Some rollback issues occurred");
            }
            Ok(())
        }
        Action::Seed(file_name) => {
            // Ensure diesel is installed
            dep_manager.ensure_installed(&["diesel"], true)?;
            
            let success = if let Some(file) = file_name {
                // Run specific seed file
                database::seed_specific_file(&file)
            } else {
                // Run all seed files
                database::seed(Some(0))
            };
            
            if !success {
                println!("Warning: Some seeding issues occurred");
            }
            Ok(())
        }
        Action::GenerateSchema => {
            // Ensure diesel is installed
            dep_manager.ensure_installed(&["diesel"], true)?;

            let success = database::generate_schema();
            if !success {
                println!("Warning: Some schema generation issues occurred");
            }
            Ok(())
        }
        Action::GenerateStructs => {
            let success = structs::generate(&current_config);
            if success {
                // Run post-generation hooks for structs
                run_post_generation_hooks(&current_config, "structs");
            } else {
                println!("Warning: Some struct generation issues occurred");
            }
            Ok(())
        }
        Action::GenerateModels => {
            let success = models::generate(&current_config);
            if success {
                // Run post-generation hooks for models
                run_post_generation_hooks(&current_config, "models");
            } else {
                println!("Warning: Some model generation issues occurred");
            }
            Ok(())
        }
        Action::EditLocaleKey => {
            locale::edit_key();
            Ok(())
        }
        Action::LocaleManager => {
            locale::launch_manager();
            Ok(())
        }
        Action::RefreshApp => {
            // Ensure diesel is installed
            dep_manager.ensure_installed(&["diesel"], true)?;

            // No global progress bar, just run each step with its own progress updates
            // Check if we're in interactive mode
            let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

            // Helper to log appropriately based on mode
            let log_message = |message: &str| {
                if is_interactive {
                    // In interactive mode, use logging system which respects quiet mode
                    let _ = crate::output::log(message);
                } else {
                    // In normal CLI mode, print to stdout
                    println!("{}", message);
                }
            };

            log_message("🔧 Rolling back migrations...");
            let rollback_ok = database::rollback_all();

            log_message("🔧 Running migrations...");
            let migrations_ok = database::migrate();

            log_message("🔧 Seeding database...");
            let seed_ok = database::seed(Some(0));

            log_message("🔧 Generating schema and structs...");
            let schema_ok = database::generate_schema();
            let structs_ok = structs::generate(&current_config);

            // Run post-generation hooks for structs if generation was successful
            if structs_ok {
                run_post_generation_hooks(&current_config, "structs");
            }

            let models_ok = models::generate(&current_config);

            // Run post-generation hooks for models if generation was successful
            if models_ok {
                run_post_generation_hooks(&current_config, "models");
            }

            if rollback_ok && migrations_ok && seed_ok && schema_ok && structs_ok && models_ok {
                log_message("\x1b[32m✔\x1b[0m App refresh complete!");
            } else {
                log_message("\x1b[32m✔\x1b[0m App refresh completed with some warnings");
            }
            Ok(())
        }
        Action::TranspileScss => {
            // Ensure sass is installed
            dep_manager.ensure_installed(&["sass"], true)?;

            assets::transpile_all_scss(&current_config).await
        }
        Action::MinifyCss => assets::minify_css_files(&current_config).await,
        Action::PublishCss => assets::publish_css(&current_config).await,
        Action::ProcessJs => assets::process_js(&current_config).await,
        Action::DownloadCdn => {
            println!("Downloading CDN assets...");
            assets::download_assets_async(&current_config).await
        }
        Action::RunDevServer => {
            if let Ok(pid) = crate::dashboard::start_server(&current_config, true) {
                println!("Development server started with PID: {}", pid);
            } else {
                Command::new("script")
                    .args(["-q", "-c", &format!("cargo run --bin {}", &current_config.project_name), "storage/logs/server.log"])
                    .spawn()?;
                println!("Development server started with cargo run --bin");
            }

            Ok(())
        }

        Action::RunProdServer => {
            if let Ok(pid) = crate::dashboard::start_server(&current_config, false) {
                println!("Production server started with PID: {}", pid);
            } else {
                // Check if the binary exists in the target/release directory
                let binary_path = format!("target/release/{}", &current_config.project_name);
                if std::path::Path::new(&binary_path).exists() {
                    // Use the compiled binary
                    Command::new("script").args(["-q", "-c", &binary_path, "storage/logs/server.log"]).spawn()?;
                    println!("Production server started using compiled binary: {}", binary_path);
                } else {
                    // Fallback to cargo run --release
                    Command::new("script")
                        .args(["-q", "-c", &format!("cargo run --release --bin {}", &current_config.project_name), "storage/logs/server.log"])
                        .spawn()?;
                    println!("Production server started with cargo run --release");
                    println!("Tip: Build with 'cargo build --release' for faster startup next time");
                }
            }

            Ok(())
        }
        Action::LaunchDashboard => {
            // Silently ensure required dependencies are installed
            dep_manager.ensure_installed(&["zellij", "diesel"], false)?;

            dashboard::launch_dashboard(&current_config)?;
            Ok(())
        }
        Action::ToggleEnvironment => {
            // Toggle environment
            let old_env = current_config.environment.clone();
            crate::configs::toggle_environment(&mut current_config)?;

            // Print confirmation message
            let env_msg = if current_config.environment == "prod" || current_config.environment == "production" {
                format!("Switched from {} to production mode", old_env)
            } else {
                format!("Switched from {} to development mode", old_env)
            };

            println!("\x1b[32m✔\x1b[0m Environment toggled: {}", env_msg);
            println!("Run `blast scss`, `blast css`, or `blast js` to rebuild assets with new settings");

            Ok(())
        }
        Action::CargoAdd(search_term) => {
            println!("Searching for crates and adding dependency...");
            crate::cargo::add_dependency(&current_config, &search_term).await
        }
        Action::CargoRemove => {
            println!("Managing dependencies...");
            crate::cargo::remove_dependency(&current_config)
        }
        Action::LogTruncate(file_name) => {
            println!("Managing log files...");
            crate::logger::ensure_log_files_exist(&current_config)?;
            crate::logger::truncate_specific_log(&current_config, file_name)
        }
        Action::Help => {
            show_help();
            Ok(())
        }
        Action::Exit => Ok(()),
    }
}
