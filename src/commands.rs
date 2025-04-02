use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::logger;
use std::error::Error;

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
    EditLocaleKey,
    ManageLocales,
    TranspileScss,
    MinifyCss,
    PublishCss,
    ProcessJs,
    DownloadCdn,

    // Server commands
    RunDevServer,
    RunProdServer,

    // Dashboard and interactive CLI commands
    LaunchDashboard,
    RunInteractiveCLI,

    // Environment commands
    ToggleEnvironment,

    // Git commands
    GitManager,
    GitStatus,
    GitPull,
    GitPush,
    GitCommit,

    // Cargo commands
    CargoAdd(String),
    CargoRemove,

    // Log commands
    LogTruncate(Option<String>),

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
        Some("dashboard") => Some(Command::LaunchDashboard),
        Some("cli") => Some(Command::RunInteractiveCLI),
        Some("toggle-env") | Some("env") => Some(Command::ToggleEnvironment),

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
        Some("locale") => Some(Command::EditLocaleKey),
        Some("locale-manager") => Some(Command::ManageLocales),
        Some("scss") => Some(Command::TranspileScss),
        Some("css") => Some(Command::MinifyCss),
        Some("publish-css") => Some(Command::PublishCss),
        Some("js") => Some(Command::ProcessJs),
        Some("cdn") => Some(Command::DownloadCdn),

        // Help
        Some("help") | Some("-h") | Some("--help") => Some(Command::Help),

        // Git commands
        Some("git") if args.len() < 3 => Some(Command::GitManager),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("status") => Some(Command::GitStatus),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("pull") => Some(Command::GitPull),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("push") => Some(Command::GitPush),
        Some("git") if args.get(2).map(|s| s.as_str()) == Some("commit") => Some(Command::GitCommit),

        // Cargo commands
        Some("cargo") if args.get(2).map(|s| s.as_str()) == Some("add") => {
            let search_term = args.get(3).map(|s| s.clone()).unwrap_or_default();
            Some(Command::CargoAdd(search_term))
        }
        Some("cargo") if args.get(2).map(|s| s.as_str()) == Some("remove") => Some(Command::CargoRemove),

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
    println!("  dashboard            Launch the interactive dashboard");
    println!("  cli                  Launch the interactive CLI");
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
    println!("  cdn                  Download assets (git clone for Materialize, CDN for others)");
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
    println!("OTHER COMMANDS:");
    println!("  new <project_name>   Create a new project");
    println!("  init                 Initialize project completely (migrations, seeds, assets, etc.)");
    println!("  help                 Show this help message");
    println!();
    println!("NOTES:");
    println!("  - Running 'blast' without arguments launches the interactive dashboard");
}

// Execute a command with config and dependency manager
pub async fn execute(cmd: Command, config: &mut Config, dep_manager: &mut DependencyManager) -> Result<(), Box<dyn Error>> {
    // Only try to reload config for commands that require an existing project
    if cmd != Command::Help && !matches!(cmd, Command::NewProject(_)) {
        // Reload config if it's been modified
        if let Err(e) = config.reload_if_modified() {
            logger::warning(&format!("Failed to reload config: {}", e))?;
        }
    }

    match cmd {
        Command::NewProject(name) => {
            logger::info(&format!("Creating new project: {}", name))?;
            crate::project::create_new_project(&name);
            logger::success(&format!("Project '{}' created successfully!", name))?;
            logger::info(&format!("Next steps:"))?;
            logger::info(&format!("  cd {}", name))?;
            logger::info(&format!("  blast init"))?;
            Ok(())
        }

        Command::InitProject => {
            // Comprehensive initialization of the project
            logger::info("Initializing project completely...")?;

            // 1. Ensure dependencies are installed
            logger::info("Checking required dependencies...")?;
            dep_manager.ensure_installed(&["diesel"], true)?;

            // 2. Database operations
            logger::info("Setting up database...")?;

            // Run migrations
            logger::info("  • Running migrations")?;
            let migrations_ok = crate::database::migrate();
            if !migrations_ok {
                logger::warning("Some migration issues occurred - check database configuration")?;
            }

            // Run seeds
            logger::info("  • Seeding database")?;
            let seed_ok = crate::database::seed(Some(0));
            if !seed_ok {
                logger::warning("Some seeding issues occurred - this may be normal for new projects")?;
            }

            // Generate schema
            logger::info("  • Generating schema")?;
            let schema_ok = crate::database::generate_schema();
            if !schema_ok {
                logger::warning("Some schema generation issues occurred")?;
            }

            // 3. Code generation
            logger::info("Generating code files...")?;

            // Generate structs
            logger::info("  • Generating structs")?;
            let structs_ok = crate::structs::generate(config);
            if !structs_ok {
                logger::warning("Some struct generation issues occurred - this may be normal for empty schemas")?;
            }

            // Generate models
            logger::info("  • Generating models")?;
            let models_ok = crate::models::generate(config);
            if !models_ok {
                logger::warning("Some model generation issues occurred - this may be normal for empty schemas")?;
            }

            // 4. Download assets
            logger::info("Setting up assets...")?;

            // Download assets (including Materialize SCSS from git)
            logger::info("  • Downloading assets")?;
            match crate::assets::download_assets_async(config).await {
                Ok(_) => logger::success("    ✓ Assets downloaded successfully")?,
                Err(e) => logger::warning(&format!("    ⚠ Some asset downloads failed: {}", e))?
            }

            // Process SCSS files
            logger::info("  • Processing SCSS files")?;
            match crate::assets::transpile_all_scss(config).await {
                Ok(_) => logger::success("    ✓ SCSS files processed successfully")?,
                Err(e) => logger::warning(&format!("    ⚠ SCSS processing error: {}", e))?
            }

            // Process CSS files
            logger::info("  • Publishing CSS files")?;
            match crate::assets::publish_css(config).await {
                Ok(_) => logger::success("    ✓ CSS files published successfully")?,
                Err(e) => logger::warning(&format!("    ⚠ CSS publishing error: {}", e))?
            }

            // Process JS files
            logger::info("  • Processing JS files")?;
            match crate::assets::process_js(config).await {
                Ok(_) => logger::success("    ✓ JS files processed successfully")?,
                Err(e) => logger::warning(&format!("    ⚠ JS processing error: {}", e))?
            }

            // 5. Final steps
            logger::success("✅ Project initialization complete! Your project is ready to run.")?;
            logger::info("Next steps:")?;
            logger::info("  1. Run 'blast run' to start the development server")?;
            logger::info("  2. Run 'blast dashboard' to launch the interactive dashboard")?;

            Ok(())
        }

        Command::RunInteractiveCLI => {
            // Use Box::pin to avoid recursive async fn issues
            return Box::pin(crate::interactive::run_interactive_cli(config.clone(), dep_manager)).await;
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

        Command::EditLocaleKey => {
            crate::locale::edit_key();
            Ok(())
        }

        Command::ManageLocales => {
            crate::locale::launch_manager();
            Ok(())
        }

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
            crate::assets::transpile_all_scss(config).await
        }

        Command::MinifyCss => crate::assets::minify_css_files(config).await,

        Command::PublishCss => crate::assets::publish_css(config).await,

        Command::ProcessJs => crate::assets::process_js(config).await,

        Command::DownloadCdn => {
            logger::info("Downloading CDN assets...")?;
            crate::assets::download_assets_async(config).await
        }

        Command::RunDevServer => {
            if let Ok(pid) = crate::dashboard::start_server(config, true) {
                logger::success(&format!("Development server started with PID: {}", pid))?;
            } else {
                let cmd = format!("cargo run --bin {}", &config.project_name);
                std::process::Command::new("script").args(["-q", "-c", &cmd, "storage/logs/server.log"]).spawn()?;
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
                    std::process::Command::new("script").args(["-q", "-c", &binary_path, "storage/logs/server.log"]).spawn()?;
                    logger::success(&format!("Production server started using compiled binary: {}", binary_path))?;
                } else {
                    let cmd = format!("cargo run --release --bin {}", &config.project_name);
                    std::process::Command::new("script").args(["-q", "-c", &cmd, "storage/logs/server.log"]).spawn()?;
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

        Command::GitManager => {
            logger::info("Launching Git manager")?;
            crate::git::launch_manager();
            Ok(())
        }

        Command::GitStatus => {
            logger::info("Git repository status:")?;
            crate::git::git_status();
            Ok(())
        }

        Command::GitPull => {
            logger::info("Pulling from remote repository...")?;
            crate::git::git_pull();
            Ok(())
        }

        Command::GitPush => {
            logger::info("Pushing to remote repository...")?;
            crate::git::git_push();
            Ok(())
        }

        Command::GitCommit => {
            logger::info("Committing changes...")?;
            crate::git::git_commit();
            Ok(())
        }

        Command::CargoAdd(search_term) => {
            logger::info(&format!("Adding dependency {}...", if search_term.is_empty() { "(interactive)" } else { &search_term }))?;
            crate::cargo::add_dependency(config, &search_term).await
        }

        Command::CargoRemove => {
            logger::info("Managing dependencies...")?;
            crate::cargo::remove_dependency(config)
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
