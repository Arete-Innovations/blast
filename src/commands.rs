use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::logger;

// Type alias for consistent error handling
type BlastResult = Result<(), String>;

// Single enum for all possible commands
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // Project commands
    NewProject(String),
    InitProject, // New command to initialize a project

    // Database commands
    NewMigration,
    Migrate,
    Rollback,
    Seed(Option<String>),
    GenerateSchema,

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

    // Dashboard and interactive CLI commands
    LaunchDashboard,
    RunInteractiveCLI,

    // Environment commands
    ToggleEnvironment,

    // Log commands
    LogTruncate(Option<String>),

    // Spark plugin commands
    AddSpark(String),

    // Cronjob commands
    CronjobsList,
    CronjobsAdd(String, i32),
    CronjobsRemove(i32),
    CronjobsToggle(i32),
    CronjobsInteractive, // Interactive TUI for cronjob management

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
        Some("new") if args.len() >= 3 => Some(Command::NewProject(args[2].clone())),
        Some("init") => Some(Command::InitProject),

        // App commands
        Some("refresh") => Some(Command::RefreshApp),
        Some("run") | Some("serve") => Some(Command::RunDevServer),
        Some("run-prod") | Some("serve-prod") => Some(Command::RunProdServer),
        Some("stop") => Some(Command::StopServer),
        Some("dashboard") => Some(Command::LaunchDashboard),
        Some("cli") => Some(Command::RunInteractiveCLI),
        Some("toggle-env") | Some("env") => Some(Command::ToggleEnvironment),

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

        // Help
        Some("help") | Some("-h") | Some("--help") => Some(Command::Help),

        // Log management
        Some("logs") | Some("log") if args.get(2).map(|s| s.as_str()) == Some("truncate") => {
            if args.len() >= 4 {
                Some(Command::LogTruncate(Some(args[3].clone())))
            } else {
                Some(Command::LogTruncate(None))
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
    println!("  dashboard            Launch the interactive dashboard");
    println!("  cli                  Launch the interactive CLI");
    println!("  toggle-env           Toggle between development and production environments");
    println!();
    println!("CRONJOB COMMANDS:");
    println!("  cronjobs             Launch interactive TUI for cronjob management");
    println!("  cronjobs interactive Launch interactive TUI for cronjob management");
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
    println!();
    println!("SPARK PLUGINS:");
    println!("  spark add <repo_url>  Add a spark plugin from a git repository");
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
    println!("  init                 Initialize project completely (migrations, seeds, assets, etc.)");
    println!("  help                 Show this help message");
    println!();
    println!("NOTES:");
    println!("  - Running 'blast' without arguments launches the interactive dashboard");
}

// Execute a command with config and dependency manager
pub fn execute(cmd: Command, config: &mut Config, dep_manager: &mut DependencyManager) -> BlastResult {
    // Only try to reload config for commands that require an existing project
    if cmd != Command::Help && !matches!(cmd, Command::NewProject(_)) {
        // Reload config if it's been modified
        if let Err(e) = config.reload_if_modified() {
            logger::warning(&format!("Failed to reload config: {}", e))?;
        }
    }

    match cmd {
        // Cronjob commands
        Command::CronjobsList => crate::cronjobs::list_cronjobs(config),

        Command::CronjobsAdd(name, interval) => crate::cronjobs::add_cronjob(config, &name, interval),

        Command::CronjobsRemove(id) => crate::cronjobs::remove_cronjob(config, id),

        Command::CronjobsToggle(id) => crate::cronjobs::toggle_cronjob(config, id),

        Command::CronjobsInteractive => {
            logger::info("Launching interactive cronjob manager...")?;
            crate::cronjobs_tui::run_cronjobs_tui(config)
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

        Command::NewProject(name) => {
            // Create the project using styled output - the function handles all output
            crate::project::create_new_project(&name);

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
            dep_manager.ensure_installed(&["diesel"], true)?;
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
            dep_manager.ensure_installed(&["diesel"], true)?;
            crate::database::new_migration();
            Ok(())
        }

        Command::Migrate => {
            dep_manager.ensure_installed(&["diesel"], true)?;
            if !crate::database::migrate() {
                logger::warning("Some migration issues occurred")?;
            }
            Ok(())
        }

        Command::Rollback => {
            dep_manager.ensure_installed(&["diesel"], true)?;
            if !crate::database::rollback_all() {
                logger::warning("Some rollback issues occurred")?;
            }
            Ok(())
        }

        Command::Seed(file_name) => {
            dep_manager.ensure_installed(&["diesel"], true)?;

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
            dep_manager.ensure_installed(&["diesel"], true)?;
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
            dep_manager.ensure_installed(&["diesel"], true)?;

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
            dep_manager.ensure_installed(&["zellij", "diesel"], false)?;
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

        Command::Help => {
            show_help();
            Ok(())
        }

        Command::Exit => Ok(()),
    }
}
