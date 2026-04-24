use std::env;
use std::process;

mod commands;
mod configs;
mod cronjobs;
mod cronjobs_tui;
mod dashboard;
mod database;
mod dependencies;
mod error;
mod interactive;
mod logger;
mod models;
mod output;
mod progress;
mod project;
mod schema_parser;
mod structs;
mod tui_viewer;

fn main() {
    let mut dep_manager = dependencies::DependencyManager::new();

    let args: Vec<String> = env::args().collect();

    let verbose_mode = args.iter().any(|arg| arg == "-v" || arg == "--verbose");

    let filtered_args: Vec<String> = args.iter()
        .filter(|arg| *arg != "-v" && *arg != "--verbose")
        .cloned()
        .collect();

    if verbose_mode {
        std::env::set_var("BLAST_VERBOSE", "1");
    }

    if let Err(e) = logger::init(logger::RuntimeMode::Cli, None) {
        eprintln!("logger init failed: {}", e);
    }

    logger::set_verbose_mode(verbose_mode);

    if filtered_args.len() > 1 {
        match commands::parse_cli_args(&filtered_args) {
            Some(cmd) => {
                match configs::get_project_info() {
                    Ok(mut config) => {
                        if let Err(e) = logger::setup_for_mode(&config, false) {
                            eprintln!("Warning: Failed to set up logging: {}", e);
                        }

                        if let Err(e) = commands::execute(cmd.clone(), &mut config, &mut dep_manager) {
                            eprintln!("Error executing command: {}", e);
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        if matches!(cmd, commands::Command::NewProject(..)) || cmd == commands::Command::Help {
                            let cwd = match std::env::current_dir() {
                                Ok(c) => c,
                                Err(io_err) => {
                                    eprintln!("Failed to get current directory: {}", io_err);
                                    process::exit(1);
                                }
                            };
                            let mut default_config = configs::Config {
                                environment: "dev".to_string(),
                                project_name: match &cmd {
                                    commands::Command::NewProject(name, _) => name.clone(),
                                    _other => "unknown".to_string(),
                                },
                                project_dir: cwd,
                                show_compiler_warnings: true,
                                last_modified: std::time::SystemTime::now(),
                            };

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

    match configs::get_project_info() {
        Ok(mut config) => {
            if let Err(e) = logger::setup_for_mode(&config, true) {
                eprintln!("Warning: Failed to set up logging: {}", e);
            }

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
