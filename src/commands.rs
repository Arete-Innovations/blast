use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::{assets, dashboard, database, locale, models, project, structs};
use std::error::Error;
use std::process::Command;

// Define action as enum to be used by both CLI and interactive mode
#[derive(PartialEq, Clone)]
pub enum Action {
    NewProject(String),
    NewMigration,
    Migrate,
    Rollback,
    GenerateSchema,
    GenerateStructs,
    GenerateModels,
    EditLocaleKey,
    LocaleManager,
    RefreshApp,
    TranspileScss,
    MinifyCss,
    ProcessJs,
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
        Some("schema") => Some(Action::GenerateSchema),

        // Asset management
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("structs") => Some(Action::GenerateStructs),
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("models") => Some(Action::GenerateModels),
        Some("locale") => Some(Action::EditLocaleKey),
        Some("locale-manager") => Some(Action::LocaleManager),
        Some("scss") => Some(Action::TranspileScss),
        Some("css") => Some(Action::MinifyCss),
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
    println!("  schema               Generate database schema");
    println!();
    println!("ASSET MANAGEMENT:");
    println!("  gen structs          Generate structs from schema");
    println!("  gen models           Generate model implementations");
    println!("  locale               Edit locale keys");
    println!("  locale-manager       Launch interactive locale management interface");
    println!("  scss                 Transpile SCSS files");
    println!("  css                  Minify CSS files");
    println!("  js                   Process JS files");
    println!("  cdn                  Download CDN assets");
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

    match action {
        Action::RunInteractiveCli => {
            let config = config.expect("Config required for interactive CLI");
            // Run the interactive CLI directly without going through execute_action
            crate::interactive::run_interactive_cli(config.clone(), dep_manager).await
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
            let config = config.expect("Config required for Catalyst manager");
            println!("Launching Catalyst.toml configuration manager...");
            crate::configs::launch_manager(config);
            Ok(())
        }
        Action::NewProject(name) => {
            // No global progress bar, just run each step with its own progress updates

            println!("🔧 Creating project structure...");
            project::create_new_project(&name);

            let cwd = std::env::current_dir()?;
            let project_dir = format!("{}/{}", cwd.display(), name);
            let config_path = format!("{}/Catalyst.toml", project_dir);
            let config = crate::configs::get_project_info_with_paths(&config_path, &project_dir)?;

            println!("🔧 Downloading CDN assets...");
            assets::download_assets_async(&config).await?;
            
            println!("🔧 Processing frontend assets...");
            assets::process_all_assets(&config).await?;

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

            println!("🔧 Generating Rust structs from database schema...");
            // Generate structs
            let structs_ok = structs::generate(&config);

            println!("🔧 Generating model implementations...");
            // Generate models
            let models_ok = models::generate(&config);

            // Determine overall success
            if migrations_ok && seeds_ok && schema_ok && structs_ok && models_ok {
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
            let config = config.expect("Config required");
            let success = structs::generate(config);
            if !success {
                println!("Warning: Some struct generation issues occurred");
            }
            Ok(())
        }
        Action::GenerateModels => {
            let config = config.expect("Config required");
            let success = models::generate(config);
            if !success {
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
            let config = config.expect("Config required");
            let structs_ok = structs::generate(config);
            let models_ok = models::generate(config);

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

            let config = config.expect("Config required");
            assets::transpile_all_scss(config).await
        }
        Action::MinifyCss => {
            let config = config.expect("Config required");
            assets::minify_css_files(config).await
        }
        Action::ProcessJs => {
            let config = config.expect("Config required");
            assets::process_js(config).await
        }
        Action::DownloadCdn => {
            let config = config.expect("Config required");
            println!("Downloading CDN assets...");
            assets::download_assets_async(config).await
        }
        Action::RunDevServer => {
            let config = config.expect("Config required");

            if let Ok(pid) = crate::dashboard::start_server(config, true) {
                println!("Development server started with PID: {}", pid);
            } else {
                Command::new("script").args(["-q", "-c", &format!("cargo run --bin {}", &config.project_name), "storage/logs/server.log"]).spawn()?;
                println!("Development server started with cargo run --bin");
            }

            Ok(())
        }

        Action::RunProdServer => {
            let config = config.expect("Config required");

            if let Ok(pid) = crate::dashboard::start_server(config, false) {
                println!("Production server started with PID: {}", pid);
            } else {
                // Check if the binary exists in the target/release directory
                let binary_path = format!("target/release/{}", &config.project_name);
                if std::path::Path::new(&binary_path).exists() {
                    // Use the compiled binary
                    Command::new("script").args(["-q", "-c", &binary_path, "storage/logs/server.log"]).spawn()?;
                    println!("Production server started using compiled binary: {}", binary_path);
                } else {
                    // Fallback to cargo run --release
                    Command::new("script")
                        .args(["-q", "-c", &format!("cargo run --release --bin {}", &config.project_name), "storage/logs/server.log"])
                        .spawn()?;
                    println!("Production server started with cargo run --release");
                    println!("Tip: Build with 'cargo build --release' for faster startup next time");
                }
            }

            Ok(())
        }
        Action::LaunchDashboard => {
            let config = config.expect("Config required");

            // Silently ensure required dependencies are installed
            dep_manager.ensure_installed(&["zellij", "diesel"], false)?;

            dashboard::launch_dashboard(config)?;
            Ok(())
        }
        Action::ToggleEnvironment => {
            let config = config.expect("Config required");

            // Toggle environment
            let old_env = config.environment.clone();
            crate::configs::toggle_environment(config)?;

            // Print confirmation message
            let env_msg = if config.environment == "prod" || config.environment == "production" {
                format!("Switched from {} to production mode", old_env)
            } else {
                format!("Switched from {} to development mode", old_env)
            };

            println!("\x1b[32m✔\x1b[0m Environment toggled: {}", env_msg);
            println!("Run `blast scss`, `blast css`, or `blast js` to rebuild assets with new settings");

            Ok(())
        }
        Action::Help => {
            show_help();
            Ok(())
        }
        Action::Exit => Ok(()),
    }
}
