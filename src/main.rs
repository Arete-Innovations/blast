use std::env;
use std::process;

mod assets;
mod commands;
mod configs;
mod cronjobs;
mod cronjobs_tui; // Interactive TUI for cronjob management
mod dashboard;
mod database;
mod dependencies;
mod interactive;
// Locale module removed
mod logger;
mod models;
mod output; // Keep temporarily until we migrate references
mod progress; // Keep temporarily until we migrate references
mod project;
mod sparks;
mod structs;

fn main() {
    // Initialize components
    let mut dep_manager = dependencies::DependencyManager::new();

    // Get command line arguments
    let args: Vec<String> = env::args().collect();
    
    // Check for verbose flag
    let verbose_mode = args.iter().any(|arg| arg == "-v" || arg == "--verbose");
    let filtered_args: Vec<String> = args.iter()
        .filter(|arg| *arg != "-v" && *arg != "--verbose")
        .cloned()
        .collect();
    
    // Initialize logger in CLI mode
    logger::init(logger::RuntimeMode::Cli, None).unwrap_or_default();
    
    // Set verbose mode if flag is present
    logger::set_verbose_mode(verbose_mode);

    // Parse CLI arguments (using filtered args without verbose flags)
    if filtered_args.len() > 1 {
        match commands::parse_cli_args(&filtered_args) {
            Some(cmd) => {
                // Load project config if needed
                match configs::get_project_info() {
                    Ok(mut config) => {
                        // Setup proper logging for one-shot commands
                        // Use CLI mode (not interactive dashboard mode)
                        if let Err(e) = logger::setup_for_mode(&config, false) {
                            eprintln!("Warning: Failed to set up logging: {}", e);
                            // Continue anyway as this shouldn't be fatal
                        }
                        
                        // Execute the command
                        if let Err(e) = commands::execute(cmd.clone(), &mut config, &mut dep_manager) {
                            eprintln!("Error executing command: {}", e);
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        // NewProject and Help don't need a project config
                        if matches!(cmd, commands::Command::NewProject(..)) || cmd == commands::Command::Help {
                            // Create a default config for these commands
                            let mut default_config = configs::Config {
                                environment: "dev".to_string(),
                                project_name: match cmd {
                                    commands::Command::NewProject(ref name, _) => name.clone(),
                                    _ => "unknown".to_string(),
                                },
                                assets: toml::Value::Table(toml::value::Table::new()),
                                project_dir: std::env::current_dir().unwrap_or_default(),
                                show_compiler_warnings: true,
                                last_modified: std::time::SystemTime::now(),
                            };

                            // For NewProject and Help, we can just use the default logger init
                            // No need to setup_for_mode as these don't write to project-specific logs
                            if let Err(e) = commands::execute(cmd, &mut default_config, &mut dep_manager) {
                                eprintln!("Error executing command: {}", e);
                                process::exit(1);
                            }
                        } else {
                            eprintln!("Failed to read project info: {}", e);
                            eprintln!("You must run this command from a project directory or use 'blast new <project_name>' to create a new project.");
                            process::exit(1);
                        }
                    }
                }
                process::exit(0);
            }
            None => {
                eprintln!("Unknown command. Run 'blast help' for usage information.");
                process::exit(1);
            }
        }
    }

    // If no arguments provided, launch dashboard by default
    match configs::get_project_info() {
        Ok(mut config) => {
            // Set up logging for interactive mode
            logger::setup_for_mode(&config, true).unwrap_or_default();

            // Launch dashboard
            if let Err(e) = commands::execute(commands::Command::LaunchDashboard, &mut config, &mut dep_manager) {
                eprintln!("Error launching dashboard: {}", e);
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to read project info: {}", e);
            eprintln!("You must run this command from a project directory or use 'blast new <project_name>' to create a new project.");
            process::exit(1);
        }
    }
}
