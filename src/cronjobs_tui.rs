use crate::configs::Config;
use crate::cronjobs::{add_cronjob, remove_cronjob, toggle_cronjob, CronjobInfo};
use crate::logger;
use chrono::{Local, TimeZone, Utc};
use std::io::Write;
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::*;
use diesel::{PgConnection, RunQueryDsl};
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use prettytable::{Table, Row, Cell, format};
use std::fs::create_dir_all;
use std::path::Path;
use std::thread;
use std::time::Duration;

// Format duration for display
fn format_duration(seconds: i32) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        format!("{}h {}m", hours, mins)
    } else {
        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        format!("{}d {}h", days, hours)
    }
}

// Format timestamp for display
fn format_timestamp(timestamp: Option<i64>) -> String {
    match timestamp {
        Some(ts) => {
            if let Some(dt) = Local.timestamp_opt(ts, 0).single() {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                "Invalid timestamp".to_string()
            }
        }
        None => "Never".to_string(),
    }
}

// Calculate next run time
fn calc_next_run(last_run: Option<i64>, timer: i32) -> String {
    match last_run {
        Some(ts) => {
            let next_ts = ts + timer as i64;
            let now = Utc::now().timestamp();
            
            if next_ts <= now {
                "Pending execution".to_string()
            } else {
                let time_left = next_ts - now;
                format_duration(time_left as i32)
            }
        },
        None => "ASAP".to_string(),
    }
}

// Establish database connection using connection string from .env
fn establish_connection(config: &Config) -> Result<PgConnection, String> {
    // Change to the project directory to ensure we pick up the correct .env file
    let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    std::env::set_current_dir(&config.project_dir).map_err(|e| format!("Failed to change to project directory: {}", e))?;
    
    // Load .env file
    dotenv().ok();
    
    // Get database URL
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL not found in .env file".to_string())?;
    
    // Restore original directory
    std::env::set_current_dir(current_dir).map_err(|e| format!("Failed to restore directory: {}", e))?;
    
    // Connect to database
    PgConnection::establish(&database_url)
        .map_err(|e| format!("Error connecting to database: {}", e))
}

// Check if cronjobs table exists
fn check_cronjobs_table(conn: &mut PgConnection) -> Result<bool, String> {
    #[derive(Debug, QueryableByName)]
    struct BoolResult {
        #[diesel(sql_type = Bool)]
        pub exists: bool,
    }

    let results = sql_query("SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'cronjobs') as exists")
        .load::<BoolResult>(conn)
        .map_err(|e| format!("Failed to check if cronjobs table exists: {}", e))?;
    
    if results.is_empty() {
        Ok(false)
    } else {
        Ok(results[0].exists)
    }
}

// Create cronjobs table if it doesn't exist
fn ensure_cronjobs_table(conn: &mut PgConnection) -> Result<(), String> {
    if !check_cronjobs_table(conn)? {
        sql_query(r#"
            CREATE TABLE IF NOT EXISTS cronjobs (
                id SERIAL PRIMARY KEY,
                name VARCHAR NOT NULL UNIQUE,
                timer INT NOT NULL,
                status VARCHAR NOT NULL DEFAULT 'active',
                last_run BIGINT
            );
            
            CREATE INDEX IF NOT EXISTS idx_cronjobs_name ON cronjobs(name);
        "#)
        .execute(conn)
        .map_err(|e| format!("Failed to create cronjobs table: {}", e))?;
        
        // Add some example cronjobs
        sql_query(r#"
            INSERT INTO cronjobs (name, timer, status) 
            VALUES 
                ('cleanup_temp_files', 3600, 'active'),
                ('send_digest_emails', 86400, 'active'),
                ('update_search_index', 43200, 'paused')
            ON CONFLICT DO NOTHING;
        "#)
        .execute(conn)
        .map_err(|e| format!("Failed to insert example cronjobs: {}", e))?;
    }
    
    Ok(())
}

// Ensure cronjob directories exist
fn ensure_cronjob_dirs(config: &Config) -> Result<(), String> {
    let cronjob_dir = Path::new(&config.project_dir).join("storage").join("cronjobs");
    create_dir_all(&cronjob_dir).map_err(|e| format!("Failed to create cronjob directories: {}", e))?;
    Ok(())
}

// Fetch all cronjobs from the database
fn fetch_cronjobs(config: &Config) -> Result<Vec<CronjobInfo>, String> {
    ensure_cronjob_dirs(config).map_err(|e| format!("Failed to create cronjob directories: {}", e))?;
    
    // Connect to database
    let mut conn = establish_connection(config)?;
    
    // Ensure cronjobs table exists
    ensure_cronjobs_table(&mut conn)?;
    
    // Fetch cronjobs
    sql_query("SELECT id, name, timer, status, last_run FROM cronjobs ORDER BY id")
        .load::<CronjobInfo>(&mut conn)
        .map_err(|e| format!("Failed to load cronjobs: {}", e))
}

// Interactive TUI for cronjob management
pub fn run_cronjobs_tui(config: &Config) -> Result<(), String> {
    let theme = ColorfulTheme::default();
    
    // Clear the screen
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    
    loop {
        // Show title
        println!("\n{}\n", Style::new().bold().underlined().apply_to("📋 CRONJOBS MANAGER"));
        
        // Fetch jobs
        let jobs = fetch_cronjobs(config)?;
        
        // Create reusable job format functions for this scope
        let format_job_for_display = |job: &CronjobInfo| -> String {
            let interval = format_duration(job.timer);
            let status = match job.status.as_str() {
                "active" => "⚡ Active",
                "paused" => "⏸️ Paused",
                "completed" => "✅ Completed",
                "failed" => "❌ Failed",
                _ => "Unknown",
            };
            
            // Truncate job name if it's too long
            let name_display = if job.name.len() > 18 {
                format!("{}...", &job.name[0..15])
            } else {
                job.name.clone()
            };
            
            format!(
                "ID: {:<3} - {:<18} (Status: {:<12}, Interval: {:<12})",
                job.id,
                name_display,
                status,
                interval
            )
        };
        
        // This formatter is no longer needed since we're using prettytable
        
        if jobs.is_empty() {
            println!("No scheduled jobs found.");
        } else {
            // Create a new table
            let mut table = Table::new();
            
            // Set the table format to look like a nice box with borders
            table.set_format(*format::consts::FORMAT_BOX_CHARS);
            
            // Add header row
            table.add_row(Row::new(vec![
                Cell::new("ID"),
                Cell::new("Name"),
                Cell::new("Status"),
                Cell::new("Interval"),
                Cell::new("Last Run"),
                Cell::new("Next Run")
            ]));
            
            // Add data rows
            for job in &jobs {
                let status_cell = match job.status.as_str() {
                    "active" => Cell::new(&job.status).style_spec("Fg=green"),
                    "paused" => Cell::new(&job.status).style_spec("Fg=yellow"),
                    "completed" => Cell::new(&job.status).style_spec("Fg=blue"),
                    "failed" => Cell::new(&job.status).style_spec("Fg=red"),
                    _ => Cell::new(&job.status),
                };
                
                // Truncate job name if it's too long
                let name_display = if job.name.len() > 25 {
                    format!("{}...", &job.name[0..22])
                } else {
                    job.name.clone()
                };
                
                let last_run = format_timestamp(job.last_run);
                let next_run = calc_next_run(job.last_run, job.timer);
                let interval = format_duration(job.timer);
                
                table.add_row(Row::new(vec![
                    Cell::new(&job.id.to_string()),
                    Cell::new(&name_display),
                    status_cell,
                    Cell::new(&interval),
                    Cell::new(&last_run),
                    Cell::new(&next_run)
                ]));
            }
            
            // Print the table
            table.printstd();
            println!("");
        }
        
        // Show menu options
        let menu_options = vec![
            "View and Manage Jobs",
            "Add New Job",
            "Back to Main Menu",
        ];
        
        let selection = FuzzySelect::with_theme(&theme)
            .with_prompt("Select an option")
            .default(0)
            .items(&menu_options)
            .interact()
            .map_err(|e| e.to_string())?;
        
        match selection {
            0 => {
                // View and manage jobs
                if jobs.is_empty() {
                    println!("No jobs to manage. Please add a job first.");
                    thread::sleep(Duration::from_secs(2));
                } else {
                    // Create entries for selection
                    let job_displays: Vec<String> = jobs.iter().map(|job| format_job_for_display(job)).collect();
                    
                    let job_selection = FuzzySelect::with_theme(&theme)
                        .with_prompt("Select a job to manage")
                        .default(0)
                        .items(&job_displays)
                        .interact()
                        .map_err(|e| e.to_string())?;
                    
                    let selected_job = &jobs[job_selection];
                    
                    let job_actions = vec![
                        format!("{} Job", if selected_job.status == "active" { "Pause" } else { "Activate" }),
                        "Remove Job".to_string(),
                        "Cancel".to_string(),
                    ];
                    
                    let action_selection = FuzzySelect::with_theme(&theme)
                        .with_prompt(&format!("Action for job '{}'", selected_job.name))
                        .default(0)
                        .items(&job_actions)
                        .interact()
                        .map_err(|e| e.to_string())?;
                    
                    match action_selection {
                        0 => {
                            // Toggle job status
                            let pb = ProgressBar::new_spinner();
                            pb.set_style(ProgressStyle::default_spinner()
                                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                                .template("{spinner:.green} {msg}")
                                .unwrap());
                            pb.set_message(format!("Toggling job '{}'...", selected_job.name));
                            
                            match toggle_cronjob(config, selected_job.id) {
                                Ok(_) => {
                                    pb.finish_with_message(format!("✅ Job '{}' toggled successfully", selected_job.name));
                                    thread::sleep(Duration::from_secs(1));
                                },
                                Err(e) => {
                                    pb.finish_with_message(format!("❌ Error: {}", e));
                                    thread::sleep(Duration::from_secs(2));
                                }
                            }
                        },
                        1 => {
                            // Remove job
                            if Confirm::with_theme(&theme)
                                .with_prompt(format!("Are you sure you want to remove job '{}'?", selected_job.name))
                                .default(false)
                                .interact()
                                .map_err(|e| e.to_string())?
                            {
                                let pb = ProgressBar::new_spinner();
                                pb.set_style(ProgressStyle::default_spinner()
                                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                                    .template("{spinner:.red} {msg}")
                                    .unwrap());
                                pb.set_message(format!("Removing job '{}'...", selected_job.name));
                                
                                match remove_cronjob(config, selected_job.id) {
                                    Ok(_) => {
                                        pb.finish_with_message(format!("✅ Job '{}' removed successfully", selected_job.name));
                                        thread::sleep(Duration::from_secs(1));
                                    },
                                    Err(e) => {
                                        pb.finish_with_message(format!("❌ Error: {}", e));
                                        thread::sleep(Duration::from_secs(2));
                                    }
                                }
                            }
                        },
                        _ => {} // Cancel, do nothing
                    }
                }
            },
            1 => {
                // Add new job
                let name: String = Input::with_theme(&theme)
                    .with_prompt("Enter job name")
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                
                if name.trim().is_empty() {
                    println!("Job name cannot be empty.");
                    thread::sleep(Duration::from_secs(2));
                    continue;
                }
                
                let interval: String = Input::with_theme(&theme)
                    .with_prompt("Enter interval in seconds (e.g. 3600 for hourly)")
                    .default("3600".into())
                    .interact_text()
                    .map_err(|e| e.to_string())?;
                
                match interval.parse::<i32>() {
                    Ok(interval_seconds) if interval_seconds > 0 => {
                        let pb = ProgressBar::new_spinner();
                        pb.set_style(ProgressStyle::default_spinner()
                            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                            .template("{spinner:.green} {msg}")
                            .unwrap());
                        pb.set_message(format!("Adding job '{}'...", name));
                        
                        match add_cronjob(config, &name, interval_seconds) {
                            Ok(_) => {
                                pb.finish_with_message(format!("✅ Job '{}' added successfully", name));
                                thread::sleep(Duration::from_secs(1));
                            },
                            Err(e) => {
                                pb.finish_with_message(format!("❌ Error: {}", e));
                                thread::sleep(Duration::from_secs(2));
                            }
                        }
                    },
                    _ => {
                        println!("Please enter a valid positive number for the interval.");
                        thread::sleep(Duration::from_secs(2));
                    }
                }
            },
            2 => break, // Exit
            _ => {} // Should not happen
        }
        
        // Clear the screen for the next iteration
        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush().map_err(|e| e.to_string())?;
    }
    
    Ok(())
}