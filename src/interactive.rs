use crate::commands::Command;
use crate::configs::Config;
use crate::dependencies::DependencyManager;
use crate::error::BlastResult;
use crate::logger;
use console::Style;
use dialoguer::{theme::ColorfulTheme, FuzzySelect};
use std::env;
use std::io::Write;

pub fn run_interactive_cli(mut config: Config, dep_manager: &mut DependencyManager) -> BlastResult<()> {
    logger::setup_for_mode(&config, true)?;

    env::set_var("BLAST_INTERACTIVE", "1");

    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush()?;

    let commands = vec![
        "[APP] Run Server",
        "[APP] Watch Server",
        "[APP] Stop Server",
        "[APP] Refresh",
        "[APP] Toggle Dev/Prod",
        "[CODEGEN] Schema",
        "[CODEGEN] Structs",
        "[CODEGEN] Models",
        "[DB] New Migration",
        "[DB] Migrate",
        "[DB] Rollback",
        "[DB] Seed",
        "[Cronjobs] Interactive Manager",
        "[Cronjobs] List Jobs",
        "[Cronjobs] Add Job",
        "[Cronjobs] Toggle Job",
        "[Cronjobs] Remove Job",
        "[LOG] Truncate Logs",
        "[Exit] Kill Session",
    ];

    let prod_style = Style::new().bold().fg(console::Color::Green);
    let dev_style = Style::new().bold().fg(console::Color::Yellow);

    loop {
        if let Err(e) = config.reload_if_modified() {
            eprintln!("config reload failed: {}", e);
        }

        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush()?;

        let prompt = if config.environment == "prod" {
            format!("{}->[{}] ", prod_style.apply_to(format!("[🚀{}]", config.environment.to_uppercase())), config.project_name)
        } else {
            format!("{}->[{}] ", dev_style.apply_to(format!("[🔧{}]", config.environment.to_uppercase())), config.project_name)
        };

        let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .items(&commands)
            .default(0)
            .interact()?;

        let cmd = match commands[selection] {
            "[APP] Refresh" => Command::RefreshApp,
            "[APP] Run Server" => {
                if config.environment == "prod" || config.environment == "production" {
                    Command::RunProdServer
                } else {
                    Command::RunDevServer
                }
            }
            "[APP] Watch Server" => Command::WatchServer,
            "[APP] Stop Server" => Command::StopServer,
            "[APP] Toggle Dev/Prod" => Command::ToggleEnvironment,

            "[CODEGEN] Schema" => Command::GenerateSchema,
            "[CODEGEN] Structs" => Command::GenerateStructs,
            "[CODEGEN] Models" => Command::GenerateModels,

            "[DB] New Migration" => Command::NewMigration,
            "[DB] Migrate" => Command::Migrate,
            "[DB] Rollback" => Command::Rollback,
            "[DB] Seed" => Command::Seed(None),

            "[Cronjobs] Interactive Manager" => Command::CronjobsInteractive,
            "[Cronjobs] List Jobs" => Command::CronjobsList,
            "[Cronjobs] Add Job" => {
                print!("\x1B[2J\x1B[1;1H");

                println!("Enter job name:");
                let mut name = String::new();
                std::io::stdin().read_line(&mut name)?;
                let name = name.trim().to_string();

                println!("Enter interval in seconds:");
                let mut interval_str = String::new();
                std::io::stdin().read_line(&mut interval_str)?;
                let interval = match interval_str.trim().parse::<i32>() {
                    Ok(n) => n,
                    Err(_e) => {
                        60
                    }
                };

                Command::CronjobsAdd(name, interval)
            }
            "[Cronjobs] Toggle Job" => {
                print!("\x1B[2J\x1B[1;1H");

                if let Err(e) = crate::cronjobs::list_cronjobs(&config) {
                    logger::warning(&format!("Failed to list jobs: {}", e))?;
                }

                println!("\nEnter job ID to toggle:");
                let mut id_str = String::new();
                std::io::stdin().read_line(&mut id_str)?;
                let id = match id_str.trim().parse::<i32>() {
                    Ok(n) => n,
                    Err(_e) => {
                        0
                    }
                };

                Command::CronjobsToggle(id)
            }
            "[Cronjobs] Remove Job" => {
                print!("\x1B[2J\x1B[1;1H");

                if let Err(e) = crate::cronjobs::list_cronjobs(&config) {
                    logger::warning(&format!("Failed to list jobs: {}", e))?;
                }

                println!("\nEnter job ID to remove:");
                let mut id_str = String::new();
                std::io::stdin().read_line(&mut id_str)?;
                let id = match id_str.trim().parse::<i32>() {
                    Ok(n) => n,
                    Err(_e) => {
                        0
                    }
                };

                Command::CronjobsRemove(id)
            }

            "[LOG] Truncate Logs" => Command::LogTruncate(None),

            "[Exit] Kill Session" => {
                logger::info("Killing Zellij session...")?;

                drop(std::process::Command::new("zellij").args(["kill-session"]).spawn());
                drop(std::process::Command::new("zellij").args(["kill-all-sessions", "-y"]).spawn());

                Command::Exit
            }
            _other_cmd => continue,
        };

        if cmd == Command::Exit {
            break;
        }

        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush()?;

        match crate::commands::execute(cmd, &mut config, dep_manager) {
            Ok(_v) => {
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            Err(e) => {
                logger::error(&format!("Command failed: {}", e))?;

                println!("\nPress Enter to continue...");
                let mut buffer = String::new();
                std::io::stdin().read_line(&mut buffer)?;
            }
        }
    }

    Ok(())
}
