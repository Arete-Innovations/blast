use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::logger;
use std::io::Write;

// Type alias for consistent error handling
type BlastResult = Result<(), String>;

// Single enum for all possible commands
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // Project commands
    NewProject(String, bool), // String = project name, bool = use dev branch
    InitProject, // New command to initialize a project

    // Database commands
    NewMigration,
    Migrate,
    Rollback,
    Seed(Option<String>),
    GenerateSchema,

    // Vessel-specific database commands
    VesselMigrate,
    VesselRefresh,

    // Code generation commands
    GenerateStructs,
    GenerateModels,

    // Asset commands
    // Locale commands removed
    TranspileScss,
    MinifyCss,
    PublishCss,
    ProcessJs,
    DownloadCdn,

    // Server commands
    RunDevServer,
    RunProdServer,
    StopServer, // New command to stop the server
    WatchServer, // Watch and auto-restart server on code changes

    // Dashboard and interactive CLI commands
    LaunchDashboard,
    RunInteractiveCLI,

    // Environment commands
    ToggleEnvironment,

    // Log commands
    LogTruncate(Option<String>),
    LogView(String), // log level to view

    // Spark plugin commands
    AddSpark(String),
    SyncSparks,

    // Cronjob commands
    CronjobsList,
    CronjobsAdd(String, i32),
    CronjobsRemove(i32),
    CronjobsToggle(i32),
    CronjobsInteractive, // Interactive TUI for cronjob management
    CronjobsLiveTable, // Live auto-refreshing table view

    // App commands
    RefreshApp,
    Help,
    #[allow(dead_code)]
    Exit,
}

// Parse CLI arguments into a Command
pub fn parse_cli_args(args: &[String]) -> Option<Command> {
    match args.get(1).map(|s| s.as_str()) {
        // Project creation
        Some("new") if args.len() >= 3 => {
            // Check if the --dev flag is present
            let use_dev_branch = args.iter().any(|arg| arg == "--dev");
            Some(Command::NewProject(args[2].clone(), use_dev_branch))
        },
        Some("init") => Some(Command::InitProject),

        // App commands
        Some("refresh") => Some(Command::RefreshApp),
        Some("run") | Some("serve") => Some(Command::RunDevServer),
        Some("run-prod") | Some("serve-prod") => Some(Command::RunProdServer),
        Some("stop") => Some(Command::StopServer),
        Some("watch") => Some(Command::WatchServer),
        Some("dashboard") => Some(Command::LaunchDashboard),
        Some("cli") => Some(Command::RunInteractiveCLI),
        Some("toggle-env") | Some("env") => Some(Command::ToggleEnvironment),

        // Vessel commands
        Some("vessel") => {
            match args.get(2).map(|s| s.as_str()) {
                Some("migrate") => Some(Command::VesselMigrate),
                Some("refresh") => Some(Command::VesselRefresh),
                _ => None,
            }
        },

        // Cronjob commands
        Some("cronjobs") => {
            match args.get(2).map(|s| s.as_str()) {
                Some("list") => Some(Command::CronjobsList),
                Some("add") if args.len() >= 5 => {
                    if let Ok(interval) = args[4].parse::<i32>() {
                        Some(Command::CronjobsAdd(args[3].clone(), interval))
                    } else {
                        None
                    }
                }
                Some("remove") if args.len() >= 4 => {
                    if let Ok(job_id) = args[3].parse::<i32>() {
                        Some(Command::CronjobsRemove(job_id))
                    } else {
                        None
                    }
                }
                Some("toggle") if args.len() >= 4 => {
                    if let Ok(job_id) = args[3].parse::<i32>() {
                        Some(Command::CronjobsToggle(job_id))
                    } else {
                        None
                    }
                }
                Some("interactive") | Some("tui") => Some(Command::CronjobsInteractive),
                Some("table") | Some("live") => Some(Command::CronjobsLiveTable),
                None => Some(Command::CronjobsInteractive), // Default to interactive mode if just "cronjobs" is provided
                _ => None,
            }
        }

        // DB commands
        Some("migration") => Some(Command::NewMigration),
        Some("migrate") => Some(Command::Migrate),
        Some("rollback") => Some(Command::Rollback),
        Some("seed") => {
            if args.len() >= 3 {
                Some(Command::Seed(Some(args[2].clone())))
            } else {
                Some(Command::Seed(None))
            }
        }
        Some("schema") => Some(Command::GenerateSchema),

        // Asset/code generation
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("structs") => Some(Command::GenerateStructs),
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("models") => Some(Command::GenerateModels),
        // Locale commands removed
        Some("scss") => Some(Command::TranspileScss),
        Some("css") => Some(Command::MinifyCss),
        Some("publish-css") => Some(Command::PublishCss),
        Some("js") => Some(Command::ProcessJs),
        Some("cdn") => Some(Command::DownloadCdn),

        // Spark plugin commands
        Some("spark") if args.get(2).map(|s| s.as_str()) == Some("add") && args.len() >= 4 => Some(Command::AddSpark(args[3].clone())),
        Some("spark") if args.get(2).map(|s| s.as_str()) == Some("sync") => Some(Command::SyncSparks),

        // Help
        Some("help") | Some("-h") | Some("--help") => Some(Command::Help),

        // Log management
        Some("logs") | Some("log") => {
            match args.get(2).map(|s| s.as_str()) {
                Some("truncate") => {
                    if args.len() >= 4 {
                        Some(Command::LogTruncate(Some(args[3].clone())))
                    } else {
                        Some(Command::LogTruncate(None))
                    }
                },
                Some("view") if args.len() >= 4 => {
                    Some(Command::LogView(args[3].clone()))
                },
                _ => None
            }
        }

        _ => None,
    }
}

// Print help information to stdout
pub fn show_help() {
    println!("Blast - Suckless Web Framework CLI");
    println!();
    println!("USAGE:");
    println!("  blast [OPTIONS] [COMMAND]");
    println!();
    println!("OPTIONS:");
    println!("  -v, --verbose       Enable verbose output (show INFO and DEBUG messages)");
    println!();
    println!("APP COMMANDS:");
    println!("  refresh              Refresh the application (rollback, migrate, seed, gen schema & structs)");
    println!("  run                  Run the development server");
    println!("  run-prod             Run the production server");
    println!("  stop                 Stop the running server");
    println!("  watch                Watch for code changes and auto-restart the server");
    println!("  dashboard            Launch the interactive dashboard");
    println!("  cli                  Launch the interactive CLI");
    println!("  toggle-env           Toggle between development and production environments");
    println!();
    println!("VESSEL COMMANDS:");
    println!("  vessel migrate       Run Vessel database migrations");
    println!("  vessel refresh       Rollback and re-run all Vessel migrations");
    println!();  
    println!("CRONJOB COMMANDS:");
    println!("  cronjobs             Launch interactive TUI for cronjob management");
    println!("  cronjobs interactive Launch interactive TUI for cronjob management");
    println!("  cronjobs table       Display live auto-refreshing table of cronjobs");
    println!("  cronjobs live        Display live auto-refreshing table of cronjobs");
    println!("  cronjobs list        List all scheduled jobs and their status");
    println!("  cronjobs add <name> <interval>  Add a new cronjob with name and interval in seconds");
    println!("  cronjobs remove <id> Remove a scheduled job by ID");
    println!("  cronjobs toggle <id> Toggle a job's active status");
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
    // Locale commands removed from help
    println!("  scss                 Transpile SCSS files");
    println!("  css                  Minify CSS files");
    println!("  publish-css          Copy CSS files from src/assets/css to public/css with optional minification");
    println!("  js                   Process JS files");
    println!("  cdn                  Download assets (git clone for Materialize, CDN for others)");
    println!();
    println!("LOG MANAGEMENT:");
    println!("  log truncate [file]   Truncate log files (all or specific file)");
    println!("  log view <level>      Interactive TUI log viewer with fuzzy search and real-time tailing");
    println!("                       Press / to search, ↑↓ to scroll, q to quit");
    println!();
    println!("SPARK PLUGINS:");
    println!("  spark add <repo_url>  Add a spark plugin from a git repository");
    println!("  spark sync            Synchronize sparks with Catalyst.toml (install missing, remove unconfigured)");
    println!("                       Dependencies listed in manifest.toml are automatically added to Cargo.toml");
    println!("                       Required environment variables are added to .env with SPARKNAME_ prefix");
    println!("                       Automatically opens an editor to replace placeholder values with actual configuration");
    println!("                       Updates Catalyst.toml with [sparks] section");
    println!("                       Sparks can also be defined in Catalyst.toml and will be installed during 'blast init'");
    println!("                       Format: [sparks]");
    println!("                               plznohac = \"https://github.com/catalyst-framework/plznohac\"");
    println!();
    println!("OTHER COMMANDS:");
    println!("  new <project_name>   Create a new project");
    println!("    --dev              Use the dev branch of the template repository");
    println!("  init                 Initialize project completely (migrations, seeds, assets, etc.)");
    println!("  help                 Show this help message");
    println!();
    println!("NOTES:");
    println!("  - Running 'blast' without arguments launches the interactive dashboard");
}

// Execute a command with config and dependency manager
pub fn execute(cmd: Command, config: &mut Config, dep_manager: &mut DependencyManager) -> BlastResult {
    // Only try to reload config for commands that require an existing project
    if cmd != Command::Help && !matches!(cmd, Command::NewProject(..)) {
        // Reload config if it's been modified
        if let Err(e) = config.reload_if_modified() {
            logger::warning(&format!("Failed to reload config: {}", e))?;
        }
    }

    match cmd {
        // Vessel commands
        Command::VesselMigrate => {
            logger::info("Running Vessel database migrations...")?;
            if crate::database::migrate_vessel_database() {
                logger::success("Vessel migrations completed successfully")?;
                Ok(())
            } else {
                logger::error("Failed to run Vessel migrations")?;
                Err("Vessel migration failed".to_string())
            }
        }
        
        Command::VesselRefresh => {
            logger::info("Refreshing Vessel database (rollback and re-run migrations)...")?;
            if crate::database::refresh_vessel_database() {
                logger::success("Vessel database refresh completed successfully")?;
                Ok(())
            } else {
                logger::error("Failed to refresh Vessel database")?;
                Err("Vessel database refresh failed".to_string())
            }
        }
        
        // Cronjob commands
        Command::CronjobsList => crate::cronjobs::list_cronjobs(config),

        Command::CronjobsAdd(name, interval) => crate::cronjobs::add_cronjob(config, &name, interval),

        Command::CronjobsRemove(id) => crate::cronjobs::remove_cronjob(config, id),

        Command::CronjobsToggle(id) => crate::cronjobs::toggle_cronjob(config, id),

        Command::CronjobsInteractive => {
            logger::info("Launching interactive cronjob manager...")?;
            crate::cronjobs_tui::run_cronjobs_tui(config)
        }
        
        Command::CronjobsLiveTable => {
            logger::info("Launching live cronjobs table view...")?;
            crate::cronjobs_tui::display_cronjobs_table(config)
        }

        Command::StopServer => {
            logger::info("Stopping running server...")?;
            if let Err(e) = crate::dashboard::stop_server() {
                logger::error(&format!("Failed to stop server: {}", e))?;
                return Err(e);
            }
            logger::success("Server stopped successfully")?;
            Ok(())
        }

        Command::AddSpark(repo_url) => {
            logger::info(&format!("Adding spark plugin from: {}", repo_url))?;
            crate::sparks::add_spark(&repo_url, config)
        }
        Command::SyncSparks => {
            logger::info("Syncing sparks from Catalyst.toml")?;
            crate::sparks::sync_sparks_from_config(config)
        }

        Command::NewProject(name, use_dev_branch) => {
            // Create the project using styled output - the function handles all output
            crate::project::create_new_project(&name, use_dev_branch);

            // No need for repetitive success message since create_new_project already prints it
            // Next steps are also already displayed in create_new_project
            Ok(())
        }

        Command::InitProject => {
            use console::style;

            // Check for verbose mode to adjust displayed information
            let is_verbose = logger::is_verbose();

            // Always show an initial message to indicate we're starting
            println!("{} Initializing project...", style("🚀").cyan());

            // Create a progress tracker for the overall process with known steps
            let total_steps = 7; // Dependencies, DB, Schema, Code Gen, Assets, SCSS/CSS/JS, Sparks
            let mut main_progress = logger::create_progress(Some(total_steps));

            // 1. Ensure dependencies are installed - less verbose messaging
            if is_verbose {
                main_progress.set_message("Project initialization (1/7): Setting up dependencies");
            }
            // We don't need to explicitly check for diesel anymore, as it will be checked when needed
            // dep_manager.ensure_installed(&["diesel"], true)?;
            main_progress.inc(1);

            // 2. Database operations - standardize primary step messages
            if is_verbose {
                main_progress.set_message("Project initialization (2/7): Setting up database");
            }

            // Run migrations - make sure they're executed fully
            main_progress.set_message("Running database migrations...");
            let migrations_ok = crate::database::migrate();
            if !migrations_ok {
                main_progress.warning("Some migration issues occurred - check database configuration")?;
            }

            // Run seeds with complete setup
            main_progress.set_message("Seeding database...");
            let seed_ok = crate::database::seed(None); // Use None to run complete seed process
            if !seed_ok {
                main_progress.warning("Some seeding issues occurred - this may be normal for new projects")?;
            }

            main_progress.inc(1);

            // 3. Generate schema - use the explicit force function to avoid any environment issues
            if is_verbose {
                main_progress.set_message("Project initialization (3/7): Generating database schema");
            } else {
                main_progress.set_message("Generating database schema...");
            }

            // Use force_regenerate_main_schema to ensure we're using the main DATABASE_URL 
            // even at this early stage
            let schema_ok = crate::database::force_regenerate_main_schema();
            if !schema_ok {
                main_progress.warning("Some schema generation issues occurred")?;
            }
            main_progress.inc(1);

            // 4. Code generation - ensure complete generation of all models and structs
            if is_verbose {
                main_progress.set_message("Project initialization (4/7): Generating code files");
            }

            // Retry struct generation if needed to ensure complete success
            main_progress.set_message("Generating structs...");
            let mut structs_ok = crate::structs::generate(config);
            if !structs_ok {
                // Retry struct generation once more after schema is confirmed generated
                structs_ok = crate::structs::generate(config);
                if !structs_ok {
                    main_progress.warning("Struct generation issues persisted - may be normal for empty schemas")?;
                }
            }

            // Retry model generation if needed to ensure complete success
            main_progress.set_message("Generating models...");
            let mut models_ok = crate::models::generate(config);
            if !models_ok {
                // Retry model generation once more with confirmed structs
                models_ok = crate::models::generate(config);
                if !models_ok {
                    main_progress.warning("Model generation issues persisted - may be normal for empty schemas")?;
                }
            }
            main_progress.inc(1);

            // 5. Download assets
            if is_verbose {
                main_progress.set_message("Project initialization (5/7): Downloading assets");
            } else {
                main_progress.set_message("Downloading assets...");
            }

            let assets_result = crate::assets::download_assets(config);
            if let Err(e) = &assets_result {
                main_progress.warning(&format!("Some asset downloads failed: {}", e))?;
            }
            main_progress.inc(1);

            // 6. Process assets (SCSS, CSS, JS)
            if is_verbose {
                main_progress.set_message("Project initialization (6/7): Processing asset files");
            } else {
                main_progress.set_message("Processing asset files...");
            }

            // Process SCSS files - these are part of final step, don't increment yet
            let scss_result = crate::assets::transpile_all_scss(config);
            if let Err(e) = &scss_result {
                main_progress.warning(&format!("SCSS processing error: {}", e))?;
            }

            // Process CSS files
            let css_result = crate::assets::publish_css(config);
            if let Err(e) = &css_result {
                main_progress.warning(&format!("CSS publishing error: {}", e))?;
            }

            // Process JS files
            let js_result = crate::assets::process_js(config);
            if let Err(e) = &js_result {
                main_progress.warning(&format!("JS processing error: {}", e))?;
            }

            main_progress.inc(1);

            // 7. Check for and install sparks from Catalyst.toml
            if is_verbose {
                main_progress.set_message("Project initialization (7/7): Installing spark plugins");
            } else {
                main_progress.set_message("Installing spark plugins...");
            }

            if let Err(e) = crate::sparks::install_sparks_from_config(config) {
                main_progress.warning(&format!("Some issues with spark installation: {}", e))?;
            }
            main_progress.inc(1);

            // CRITICAL: Force regenerate schema from main DATABASE_URL to override any spark changes
            main_progress.set_message("Ensuring schema is generated for main database...");
            
            // Always force regenerate the schema as the last step to ensure it's correct
            let schema_fixed = crate::database::force_regenerate_main_schema();
            if !schema_fixed {
                main_progress.warning("Failed to force-regenerate schema from main database. The schema may be incorrect.")?;
            } else {
                main_progress.success("Schema has been correctly regenerated from main DATABASE_URL");
            }
            
            // Re-run struct and model generation to ensure they match the fixed schema
            main_progress.set_message("Regenerating structs and models from fixed schema...");
            let structs_regenerated = crate::structs::generate(config);
            let models_regenerated = crate::models::generate(config);
            
            if !structs_regenerated || !models_regenerated {
                main_progress.warning("Failed to regenerate some structs or models. You may need to run 'blast gen structs' and 'blast gen models' manually.")?;
            } else {
                main_progress.success("Structs and models have been regenerated successfully");
            }
            
            // Initialize Vessel database
            main_progress.set_message("Initializing Vessel database...");
            if crate::database::initialize_vessel_database() {
                main_progress.success("Vessel database initialized successfully");
            } else {
                main_progress.warning("Failed to initialize Vessel database. You may need to run 'blast vessel migrate' manually.")?;
            }

            // Finish with success message - clear the progress bar first
            main_progress.success("Project initialization complete!");

            // Show next steps for the user with consistent styling
            println!("{} Your project is ready to run! {}", style("🎉").green(), style("🚀").green());

            println!("\nNext steps:");
            println!("  {} Run 'blast run' to start the development server", style("1.").cyan());
            println!("  {} Run 'blast dashboard' to launch the interactive dashboard", style("2.").cyan());

            Ok(())
        }

        Command::RunInteractiveCLI => {
            // Now sync, no need for Box::pin
            return crate::interactive::run_interactive_cli(config.clone(), dep_manager);
        }

        Command::NewMigration => {
            // We no longer need this, as the function will check for diesel with PostgreSQL features
            // dep_manager.ensure_installed(&["diesel"], true)?;
            crate::database::new_migration();
            Ok(())
        }

        Command::Migrate => {
            // We no longer need this, as the function will check for diesel with PostgreSQL features
            // dep_manager.ensure_installed(&["diesel"], true)?;
            if !crate::database::migrate() {
                logger::warning("Some migration issues occurred")?;
            }
            Ok(())
        }

        Command::Rollback => {
            // We no longer need this, as the function will check for diesel with PostgreSQL features
            // dep_manager.ensure_installed(&["diesel"], true)?;
            if !crate::database::rollback_all() {
                logger::warning("Some rollback issues occurred")?;
            }
            Ok(())
        }

        Command::Seed(file_name) => {
            // We no longer need this, as the function will check for diesel with PostgreSQL features
            // dep_manager.ensure_installed(&["diesel"], true)?;

            let success = if let Some(file) = file_name {
                crate::database::seed_specific_file(&file)
            } else {
                crate::database::seed(Some(0))
            };

            if !success {
                logger::warning("Some seeding issues occurred")?;
            }
            Ok(())
        }

        Command::GenerateSchema => {
            // We no longer need this, as the function will check for diesel with PostgreSQL features
            // dep_manager.ensure_installed(&["diesel"], true)?;
            if !crate::database::generate_schema() {
                logger::warning("Some schema generation issues occurred")?;
            }
            Ok(())
        }

        Command::GenerateStructs => {
            if !crate::structs::generate(config) {
                logger::warning("Some struct generation issues occurred")?;
            }
            Ok(())
        }

        Command::GenerateModels => {
            if !crate::models::generate(config) {
                logger::warning("Some model generation issues occurred")?;
            }
            Ok(())
        }

        // Locale commands removed
        Command::RefreshApp => {
            // App refresh involves multiple steps
            let mut progress = logger::create_progress(None);

            // Database operations
            // We don't need to explicitly check for diesel anymore
            // dep_manager.ensure_installed(&["diesel"], true)?;

            progress.set_message("Rolling back migrations...");
            let rollback_ok = crate::database::rollback_all();

            progress.set_message("Running migrations...");
            let migrations_ok = crate::database::migrate();

            progress.set_message("Seeding database...");
            let seed_ok = crate::database::seed(Some(0));

            progress.set_message("Generating schema...");
            let schema_ok = crate::database::generate_schema();

            progress.set_message("Generating structs...");
            let structs_ok = crate::structs::generate(config);

            progress.set_message("Generating models...");
            let models_ok = crate::models::generate(config);

            if rollback_ok && migrations_ok && seed_ok && schema_ok && structs_ok && models_ok {
                progress.success("App refresh complete!");
            } else {
                progress.error("App refresh completed with some issues");
            }

            Ok(())
        }

        Command::TranspileScss => {
            // Use the built-in Rust sass-rs crate, no external dependency needed
            crate::assets::transpile_all_scss(config)
        }

        Command::MinifyCss => crate::assets::minify_css_files(config),

        Command::PublishCss => crate::assets::publish_css(config),

        Command::ProcessJs => crate::assets::process_js(config),

        Command::DownloadCdn => {
            // The download_assets_async function now handles environment mode setting internally
            // to ensure consistent behavior between CLI and dashboard modes
            match crate::assets::download_assets(config) {
                Ok(_) => {
                    // Success already logged by the function
                    Ok(())
                }
                Err(e) => {
                    // Error handling - the function will already log specific errors
                    logger::error(&format!("Failed to download CDN assets: {}", e))?;
                    Err(e)
                }
            }
        }

        Command::RunDevServer => {
            if let Ok(pid) = crate::dashboard::start_server(config, true) {
                logger::success(&format!("Development server started with PID: {}", pid))?;
            } else {
                let cmd = format!("cargo run --bin {}", &config.project_name);
                std::process::Command::new("script").args(["-q", "-c", &cmd, "storage/logs/server.log"]).spawn().map_err(|e| e.to_string())?;
                logger::success("Development server started with cargo run")?;
            }
            Ok(())
        }

        Command::RunProdServer => {
            if let Ok(pid) = crate::dashboard::start_server(config, false) {
                logger::success(&format!("Production server started with PID: {}", pid))?;
            } else {
                // Check if binary exists
                let binary_path = format!("target/release/{}", &config.project_name);
                if std::path::Path::new(&binary_path).exists() {
                    std::process::Command::new("script")
                        .args(["-q", "-c", &binary_path, "storage/logs/server.log"])
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    logger::success(&format!("Production server started using compiled binary: {}", binary_path))?;
                } else {
                    let cmd = format!("cargo run --release --bin {}", &config.project_name);
                    std::process::Command::new("script").args(["-q", "-c", &cmd, "storage/logs/server.log"]).spawn().map_err(|e| e.to_string())?;
                    logger::success("Production server started with cargo run --release")?;
                    logger::info("Tip: Build with 'cargo build --release' for faster startup next time")?;
                }
            }
            Ok(())
        }

        Command::LaunchDashboard => {
            // Only need to ensure zellij is installed, diesel will be checked when needed
            dep_manager.ensure_installed(&["zellij"], false)?;
            crate::dashboard::launch_dashboard(config)?;
            Ok(())
        }

        Command::ToggleEnvironment => {
            // Toggle environment
            config.toggle_environment()?;
            logger::info("Run `blast scss`, `blast css`, or `blast js` to rebuild assets with new settings")?;
            Ok(())
        }

        Command::LogTruncate(file_name) => {
            logger::info("Managing log files...")?;
            crate::logger::ensure_log_files_exist(config)?;
            crate::logger::truncate_specific_log(config, file_name)
        }

        Command::LogView(level) => {
            // Use the new TUI viewer with fuzzy search and real-time tailing
            if let Err(e) = crate::tui_viewer::run_tui_log_viewer(&level, config) {
                // Fallback to the old viewer if TUI fails
                logger::warning(&format!("TUI viewer failed ({}), falling back to simple viewer", e))?;
                crate::logger::view_logs_enhanced(&level, config)
            } else {
                Ok(())
            }
        }

        Command::Help => {
            show_help();
            Ok(())
        }

        Command::WatchServer => {
            // Ensure cargo-watch is installed
            dep_manager.ensure_installed(&["cargo-watch"], true)?;

            logger::info(&format!("Starting watch mode for {}", &config.project_name))?;

            // Kill any existing server process
            crate::dashboard::stop_server().map_err(|e| e.to_string())?;

            // Create logs directory if it doesn't exist
            let logs_dir = config.project_dir.join("storage").join("logs");
            std::fs::create_dir_all(&logs_dir).map_err(|e| e.to_string())?;

            // Create storage directory for PIDs
            let blast_dir = config.project_dir.join("storage").join("blast");
            std::fs::create_dir_all(&blast_dir).map_err(|e| e.to_string())?;

            // Update Rocket.toml configuration - use dev mode for watch
            if let Err(e) = crate::project::update_rocket_config(config, true) {
                // If the config files don't exist yet, just log a warning and continue
                logger::warning(&format!("Failed to update Rocket.toml configuration: {}", e))?;
            }

            // Get log path
            let server_log_path = logs_dir.join("server.log");

            // Open log file (make sure it exists)
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&server_log_path)
                .map_err(|e| e.to_string())?;

            // Construct the command with the project name
            let watch_cmd = format!(
                "nohup script -q -f -c \"cargo watch -x 'run --bin {}'\" storage/logs/server.log </dev/null >/dev/null 2>&1 & echo $!",
                &config.project_name
            );

            // Execute the command
            let output = std::process::Command::new("bash")
                .args(["-c", &watch_cmd])
                .output()
                .map_err(|e| format!("Failed to start cargo-watch: {}", e))?;

            // Capture the PID from the output of the command
            let pid_str = String::from_utf8_lossy(&output.stdout);
            let pid = pid_str.trim().parse::<u32>().map_err(|_| "Failed to parse PID".to_string())?;

            // Store the PID
            let pid_file_path = blast_dir.join("server.pid");
            std::fs::write(&pid_file_path, pid.to_string()).map_err(|e| e.to_string())?;

            // Log to the server log
            let timestamp = chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]");
            let mut server_log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&server_log_path)
                .map_err(|e| e.to_string())?;

            writeln!(server_log, "{} Using development configuration", timestamp)
                .map_err(|e| e.to_string())?;
            writeln!(server_log, "{} Watch mode started with PID: {}", timestamp, pid)
                .map_err(|e| e.to_string())?;

            logger::success(&format!("Watch mode started with PID: {}. Server will restart automatically when code changes.", pid))?;
            Ok(())
        },
        
        Command::Exit => Ok(()),
    }
}
