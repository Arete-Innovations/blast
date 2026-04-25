use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::error::{BlastError, BlastResult};
use crate::logger;
use std::io::Write;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    NewProject(String, bool),
    InitProject,

    NewMigration,
    Migrate,
    Rollback,
    Seed(Option<String>),
    GenerateSchema,

    GenerateStructs,
    GenerateModels,

    RunDevServer,
    RunProdServer,
    StopServer,
    WatchServer,

    LaunchDashboard,
    RunInteractiveCLI,

    ToggleEnvironment,

    LogTruncate(Option<String>),
    LogView(String),

    FusesList,
    FusesToggle(String),
    FusesInteractive,
    FusesLiveTable,
    FusesRun(String),
    FusesLogs(String),

    Build,
    Package,

    RefreshApp,
    Help,
    Exit,

    GenTable,
    GenMigrationCustom(String),
    GenInteractivePicker,
    Check { verbose: bool },
    Arsenal,
    ArsenalServe,
    GenFrontend,
}

pub fn parse_cli_args(args: &[String]) -> Option<Command> {
    match args.get(1).map(|s| s.as_str()) {
        Some("new") if args.len() >= 3 => {
            let use_dev_branch = args.iter().any(|arg| arg == "--dev");
            Some(Command::NewProject(args[2].clone(), use_dev_branch))
        },
        Some("init") => Some(Command::InitProject),

        Some("refresh") => Some(Command::RefreshApp),
        Some("run") | Some("serve") => Some(Command::RunDevServer),
        Some("run-prod") | Some("serve-prod") => Some(Command::RunProdServer),
        Some("stop") => Some(Command::StopServer),
        Some("watch") => Some(Command::WatchServer),
        Some("dashboard") => Some(Command::LaunchDashboard),
        Some("cli") => Some(Command::RunInteractiveCLI),
        Some("toggle-env") | Some("env") => Some(Command::ToggleEnvironment),

        Some("fuses") => {
            match args.get(2).map(|s| s.as_str()) {
                Some("list") => Some(Command::FusesList),
                Some("toggle") if args.len() >= 4 => {
                    Some(Command::FusesToggle(args[3].clone()))
                }
                Some("run") if args.len() >= 4 => {
                    Some(Command::FusesRun(args[3].clone()))
                }
                Some("logs") if args.len() >= 4 => {
                    Some(Command::FusesLogs(args[3].clone()))
                }
                Some("interactive") | Some("tui") => Some(Command::FusesInteractive),
                Some("table") | Some("live") => Some(Command::FusesLiveTable),
                None => Some(Command::FusesInteractive),
                _sub => None,
            }
        }

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

        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("structs") => Some(Command::GenerateStructs),
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("models") => Some(Command::GenerateModels),
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("table") => Some(Command::GenTable),
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("frontend") => Some(Command::GenFrontend),
        Some("gen") if args.get(2).map(|s| s.as_str()) == Some("migration") => {
            parse_gen_migration(args)
        }
        Some("gen") if args.get(2).is_none() => Some(Command::GenInteractivePicker),

        Some("build") => Some(Command::Build),
        Some("package") => Some(Command::Package),

        Some("check") => {
            let verbose = logger::is_verbose();
            Some(Command::Check { verbose })
        }

        Some("arsenal") => match args.get(2).map(|s| s.as_str()) {
            Some("serve") => Some(Command::ArsenalServe),
            None => Some(Command::Arsenal),
            Some(_other) => None,
        },

        Some("help") | Some("-h") | Some("--help") => Some(Command::Help),

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
                _sub => None
            }
        }

        _sub => None,
    }
}

fn parse_gen_migration(args: &[String]) -> Option<Command> {
    let custom = args.iter().any(|arg| arg == "--custom");
    if !custom {
        return None;
    }
    let name = args
        .iter()
        .skip(3)
        .find(|arg| !arg.starts_with("--"))
        .cloned()?;
    Some(Command::GenMigrationCustom(name))
}

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
    println!("FUSES COMMANDS:");
    println!("  fuses                Launch interactive TUI for fuse management");
    println!("  fuses list           List all registered fuses and their status");
    println!("  fuses toggle <name>  Toggle a fuse's enabled flag by name");
    println!("  fuses run <name>     Trigger immediate run of a fuse (bypass schedule)");
    println!("  fuses logs <name>    Show recent run log entries for a fuse");
    println!();
    println!("DATABASE COMMANDS:");
    println!("  migration            Create a new migration");
    println!("  migrate              Run all pending migrations");
    println!("  rollback             Rollback all migrations");
    println!("  seed [file]          Run database seeds (all or specific file)");
    println!("  schema               Generate database schema");
    println!();
    println!("CODE GENERATION:");
    println!("  gen                  Interactive picker for codegen targets");
    println!("  gen structs          Generate structs from schema");
    println!("  gen models           Generate model implementations");
    println!("  gen table            Interactive wizard to author a CREATE TABLE migration");
    println!("  gen migration --custom <name>  Scaffold an empty migration and open $EDITOR");
    println!("  gen frontend         Generate FE artifacts (TS validators, list query helpers) from primer IR");
    println!();
    println!("LOG MANAGEMENT:");
    println!("  log truncate [file]   Truncate log files (all or specific file)");
    println!("  log view <level>      Interactive TUI log viewer with fuzzy search and real-time tailing");
    println!("                       Press / to search, ↑↓ to scroll, q to quit");
    println!();
    println!("BUILD COMMANDS:");
    println!("  build                Production build (lint + frontend + cargo release)");
    println!("  package              Tarball binary + dist + .env.example + systemd unit");
    println!();
    println!("LINT COMMANDS:");
    println!("  check                Run frontend lint engine (Governor)");
    println!("    --verbose          Extra diagnostic output");
    println!();
    println!("ARSENAL:");
    println!("  arsenal              Scan source and write target/arsenal.json");
    println!("  arsenal serve        Serve capability inventory over MCP stdio");
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

pub fn execute(cmd: Command, config: &mut Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    if cmd != Command::Help && !matches!(cmd, Command::NewProject(..)) {
        if let Err(e) = config.reload_if_modified() {
            logger::warning(&format!("Failed to reload config: {}", e))?;
        }
    }

    match cmd {
        Command::FusesList => crate::fuses::list_fuses(config),

        Command::FusesToggle(name) => crate::fuses::toggle_fuse(config, &name),

        Command::FusesRun(name) => {
            logger::info(&format!("Triggering immediate run of fuse '{}'...", name))?;
            crate::fuses::run_fuse(config, &name)
        }

        Command::FusesLogs(name) => crate::fuses::logs_fuse(config, &name),

        Command::FusesInteractive => {
            logger::info("Launching interactive fuses manager...")?;
            crate::fuses_tui::run_fuses_tui(config)
        }

        Command::FusesLiveTable => {
            logger::info("Launching live fuses table view...")?;
            crate::fuses_tui::display_fuses_table(config)
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

        Command::NewProject(name, use_dev_branch) => {
            crate::project::create_new_project(&name, use_dev_branch);

            Ok(())
        }

        Command::InitProject => {
            use console::style;

            let is_verbose = logger::is_verbose();

            println!("{} Initializing project...", style("🚀").cyan());

            let total_steps = 4;
            let mut main_progress = logger::create_progress(Some(total_steps));

            if is_verbose {
                main_progress.set_message("Project initialization (1/4): Setting up dependencies");
            }
            main_progress.inc(1);

            if is_verbose {
                main_progress.set_message("Project initialization (2/4): Setting up database");
            }

            main_progress.set_message("Running database migrations...");
            let migrations_ok = crate::database::migrate();
            if !migrations_ok {
                main_progress.warning("Some migration issues occurred - check database configuration")?;
            }

            main_progress.set_message("Seeding database...");
            let seed_ok = crate::database::seed(None);
            if !seed_ok {
                main_progress.warning("Some seeding issues occurred - this may be normal for new projects")?;
            }

            main_progress.inc(1);

            if is_verbose {
                main_progress.set_message("Project initialization (3/4): Generating database schema");
            } else {
                main_progress.set_message("Generating database schema...");
            }

            let schema_ok = crate::database::force_regenerate_main_schema();
            if !schema_ok {
                main_progress.warning("Some schema generation issues occurred")?;
            }
            main_progress.inc(1);

            if is_verbose {
                main_progress.set_message("Project initialization (4/4): Generating code files");
            }

            main_progress.set_message("Generating structs...");
            let mut structs_ok = crate::structs::generate(config);
            if !structs_ok {
                structs_ok = crate::structs::generate(config);
                if !structs_ok {
                    main_progress.warning("Struct generation issues persisted - may be normal for empty schemas")?;
                }
            }

            main_progress.set_message("Generating models...");
            let mut models_ok = crate::models::generate(config);
            if !models_ok {
                models_ok = crate::models::generate(config);
                if !models_ok {
                    main_progress.warning("Model generation issues persisted - may be normal for empty schemas")?;
                }
            }
            main_progress.inc(1);

            main_progress.set_message("Ensuring schema is generated for main database...");

            let schema_fixed = crate::database::force_regenerate_main_schema();
            if !schema_fixed {
                main_progress.warning("Failed to force-regenerate schema from main database. The schema may be incorrect.")?;
            } else {
                main_progress.success("Schema has been correctly regenerated from main DATABASE_URL");
            }

            main_progress.set_message("Regenerating structs and models from fixed schema...");
            let structs_regenerated = crate::structs::generate(config);
            let models_regenerated = crate::models::generate(config);

            if !structs_regenerated || !models_regenerated {
                main_progress.warning("Failed to regenerate some structs or models. You may need to run 'blast gen structs' and 'blast gen models' manually.")?;
            } else {
                main_progress.success("Structs and models have been regenerated successfully");
            }

            main_progress.success("Project initialization complete!");

            println!("{} Your project is ready to run! {}", style("🎉").green(), style("🚀").green());

            println!("\nNext steps:");
            println!("  {} Run 'blast run' to start the development server", style("1.").cyan());
            println!("  {} Run 'blast dashboard' to launch the interactive dashboard", style("2.").cyan());

            Ok(())
        }

        Command::RunInteractiveCLI => {
            return crate::interactive::run_interactive_cli(config.clone(), dep_manager);
        }

        Command::NewMigration => {
            crate::database::new_migration();
            Ok(())
        }

        Command::Migrate => {
            if !crate::database::migrate() {
                logger::warning("Some migration issues occurred")?;
            }
            Ok(())
        }

        Command::Rollback => {
            if !crate::database::rollback_all() {
                logger::warning("Some rollback issues occurred")?;
            }
            Ok(())
        }

        Command::Seed(file_name) => {
            let success = match file_name {
                Some(file) => crate::database::seed_specific_file(&file),
                None => crate::database::seed(Some(0)),
            };

            if !success {
                logger::warning("Some seeding issues occurred")?;
            }
            Ok(())
        }

        Command::GenerateSchema => {
            if !crate::database::generate_schema() {
                logger::warning("Some schema generation issues occurred")?;
            }
            let schema_path = config.project_dir.join("src/database/schema.rs");
            match crate::schema_parser::parse_schema(&schema_path) {
                Ok(tables) => {
                    let col_count: usize = tables.iter().map(|t| t.columns.len()).sum();
                    let nullable_count: usize = tables
                        .iter()
                        .flat_map(|t| t.columns.iter())
                        .filter(|c| c.nullable)
                        .count();
                    logger::info(&format!(
                        "schema: {} tables, {} columns ({} nullable) — pk fields: {}",
                        tables.len(),
                        col_count,
                        nullable_count,
                        tables
                            .iter()
                            .map(|t| format!("{}({})", t.name, t.primary_key.join(",")))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ))?;
                    for t in &tables {
                        logger::debug(&format!(
                            "  {} cols: {}",
                            t.name,
                            t.columns
                                .iter()
                                .map(|c| format!(
                                    "{}:{}{}",
                                    c.name,
                                    c.diesel_type,
                                    if c.nullable { "?" } else { "" }
                                ))
                                .collect::<Vec<_>>()
                                .join(", "),
                        ))?;
                    }
                }
                Err(e) => {
                    logger::warning(&format!("schema parse warning: {}", e))?;
                }
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

        Command::Build => {
            crate::build::run_build(config)
        }

        Command::Package => {
            crate::build::run_package(config)
        }

        Command::RefreshApp => {
            let mut progress = logger::create_progress(None);

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

        Command::RunDevServer => {
            match crate::dashboard::start_server(config, true) {
                Ok(pid) => {
                    logger::success(&format!("Development server started with PID: {}", pid))?;
                }
                Err(_started) => {
                    let cmd = format!("cargo run --bin {}", &config.project_name);
                    std::process::Command::new("script").args(["-q", "-c", &cmd, "storage/logs/server.log"]).spawn()?;
                    logger::success("Development server started with cargo run")?;
                }
            }
            Ok(())
        }

        Command::RunProdServer => {
            match crate::dashboard::start_server(config, false) {
                Ok(pid) => {
                    logger::success(&format!("Production server started with PID: {}", pid))?;
                }
                Err(_started) => {
                    let binary_path = format!("target/release/{}", &config.project_name);
                    if std::path::Path::new(&binary_path).exists() {
                        std::process::Command::new("script")
                            .args(["-q", "-c", &binary_path, "storage/logs/server.log"])
                            .spawn()?;
                        logger::success(&format!("Production server started using compiled binary: {}", binary_path))?;
                    } else {
                        let cmd = format!("cargo run --release --bin {}", &config.project_name);
                        std::process::Command::new("script").args(["-q", "-c", &cmd, "storage/logs/server.log"]).spawn()?;
                        logger::success("Production server started with cargo run --release")?;
                        logger::info("Tip: Build with 'cargo build --release' for faster startup next time")?;
                    }
                }
            }
            Ok(())
        }

        Command::LaunchDashboard => {
            dep_manager.ensure_installed(&["zellij"], false)?;
            crate::dashboard::launch_dashboard(config)?;
            Ok(())
        }

        Command::ToggleEnvironment => {
            config.toggle_environment()?;
            logger::info("Run `blast scss`, `blast css`, or `blast js` to rebuild assets with new settings")?;
            Ok(())
        }

        Command::LogTruncate(file_name) => {
            logger::info("Managing log files...")?;
            crate::logger::ensure_log_files_exist(config)?;
            crate::logger::truncate_specific_log(config, file_name)        }

        Command::LogView(level) => {
            if let Err(e) = crate::tui_viewer::run_tui_log_viewer(&level, config) {
                logger::warning(&format!("TUI viewer failed ({}), falling back to simple viewer", e))?;
                crate::logger::view_logs_enhanced(&level, config)            } else {
                Ok(())
            }
        }

        Command::Help => {
            show_help();
            Ok(())
        }

        Command::WatchServer => {
            dep_manager.ensure_installed(&["cargo-watch"], true)?;

            logger::info(&format!("Starting watch mode for {}", &config.project_name))?;

            crate::dashboard::stop_server()?;

            let logs_dir = config.project_dir.join("storage").join("logs");
            std::fs::create_dir_all(&logs_dir)?;

            let blast_dir = config.project_dir.join("storage").join("blast");
            std::fs::create_dir_all(&blast_dir)?;

            let server_log_path = logs_dir.join("server.log");

            let _touch = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&server_log_path)?;
            drop(_touch);

            let watch_cmd = format!(
                "nohup script -q -f -c \"cargo watch -x 'run --bin {}'\" storage/logs/server.log </dev/null >/dev/null 2>&1 & echo $!",
                &config.project_name
            );

            let output = std::process::Command::new("bash")
                .args(["-c", &watch_cmd])
                .output()?;

            let pid_str = String::from_utf8_lossy(&output.stdout);
            let pid = pid_str.trim().parse::<u32>().map_err(|e| BlastError::Invalid(e.to_string()))?;

            let pid_file_path = blast_dir.join("server.pid");
            std::fs::write(&pid_file_path, pid.to_string())?;

            let timestamp = chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]");
            let mut server_log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&server_log_path)?;

            writeln!(server_log, "{} Using development configuration", timestamp)?;
            writeln!(server_log, "{} Watch mode started with PID: {}", timestamp, pid)?;

            logger::success(&format!("Watch mode started with PID: {}. Server will restart automatically when code changes.", pid))?;
            Ok(())
        },
        
        Command::Check { verbose } => {
            let project_root = config.project_dir.clone();
            let outcome = crate::governor::run_check(&project_root, verbose)?;
            print!("{}", outcome.output);
            std::io::stdout().flush()?;
            if outcome.violation_count > 0 {
                std::process::exit(1);
            }
            Ok(())
        }

        Command::Arsenal => {
            logger::info("Scanning source for arsenal report...")?;
            let report = crate::arsenal::scanner::scan(&config.project_dir)?;
            crate::arsenal::report::write_report(&report, &config.project_dir)?;
            let total_entries: usize = report.layers.values().map(|v| v.len()).sum();
            logger::success(&format!(
                "arsenal: {} entries across {} layers, {} routes -> target/arsenal.json",
                total_entries,
                report.layers.len(),
                report.routes.len(),
            ))?;
            Ok(())
        }

        Command::ArsenalServe => {
            let report = crate::arsenal::scanner::scan(&config.project_dir)?;
            crate::arsenal::mcp::serve(report)
        }

        Command::Exit => Ok(()),

        Command::GenTable => {
            crate::gen_table::run()
        }

        Command::GenMigrationCustom(name) => {
            crate::gen_migration::run_custom(&name)
        }

        Command::GenInteractivePicker => {
            run_gen_picker(config, dep_manager)
        }

        Command::GenFrontend => {
            logger::info("Generating frontend artifacts from primer IR...")?;
            crate::codegen::run_frontend(&config.project_dir)?;
            logger::success("Frontend codegen complete")?;
            Ok(())
        }
    }
}

fn run_gen_picker(
    config: &mut Config,
    dep_manager: &mut DependencyManager,
) -> BlastResult<()> {
    use dialoguer::{theme::ColorfulTheme, FuzzySelect};

    let items = vec![
        "Generate schema (diesel print-schema)",
        "Generate structs",
        "Generate models",
        "Generate table (interactive wizard)",
        "Generate custom migration (--custom)",
        "Generate frontend (validators + list query helpers)",
    ];

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("blast gen — pick a target")
        .items(&items)
        .default(0)
        .interact()?;

    match selection {
        0 => execute(Command::GenerateSchema, config, dep_manager),
        1 => execute(Command::GenerateStructs, config, dep_manager),
        2 => execute(Command::GenerateModels, config, dep_manager),
        3 => execute(Command::GenTable, config, dep_manager),
        4 => {
            let name: String = dialoguer::Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Migration name (snake_case)")
                .interact_text()?;
            execute(Command::GenMigrationCustom(name), config, dep_manager)
        }
        5 => execute(Command::GenFrontend, config, dep_manager),
        other => Err(BlastError::Invalid(format!(
            "unknown gen picker selection: {}",
            other
        ))),
    }
}
