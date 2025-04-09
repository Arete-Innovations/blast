use crate::configs::Config;
use crate::logger;
use chrono::{Local, TimeZone, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::*;
use diesel::{PgConnection, RunQueryDsl};
use dotenv::dotenv;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

// Structure to hold cronjob information for database queries
#[derive(Debug, QueryableByName)]
pub struct CronjobInfo {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Integer)]
    pub timer: i32,
    #[diesel(sql_type = Text)]
    pub status: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub last_run: Option<i64>,
}

// Boolean result type for database queries
#[derive(Debug, QueryableByName)]
pub struct BoolResult {
    #[diesel(sql_type = Bool)]
    pub exists: bool,
}

// String result type for database queries
#[derive(Debug, QueryableByName)]
pub struct StringResult {
    #[diesel(sql_type = Text)]
    pub result: String,
}

// Data for display in the CLI
pub struct CronjobDisplay {
    pub id: i32,
    pub name: String,
    pub interval: String,
    pub status: String,
    pub last_run: String,
    pub next_run: String,
}

// Ensure cronjob directories exist
fn ensure_cronjob_dirs(config: &Config) -> io::Result<()> {
    let cronjob_dir = Path::new(&config.project_dir).join("storage").join("cronjobs");
    fs::create_dir_all(&cronjob_dir)?;

    // Create log files if they don't exist
    let execution_log = cronjob_dir.join("execution.log");
    let errors_log = cronjob_dir.join("errors.log");

    if !execution_log.exists() {
        let mut file = File::create(&execution_log)?;
        writeln!(file, "--- Cronjob Execution Log ---")?;
    }

    if !errors_log.exists() {
        let mut file = File::create(&errors_log)?;
        writeln!(file, "--- Cronjob Error Log ---")?;
    }

    Ok(())
}

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
        }
        None => "ASAP".to_string(),
    }
}

// Log to the cronjob execution log
fn log_to_execution(config: &Config, message: &str) -> Result<(), String> {
    let log_path = Path::new(&config.project_dir).join("storage").join("cronjobs").join("execution.log");

    let mut file = OpenOptions::new().create(true).append(true).open(log_path).map_err(|e| format!("Failed to open execution log: {}", e))?;

    let timestamp = Local::now().format("[%Y-%m-%d %H:%M:%S]");
    writeln!(file, "{} {}", timestamp, message).map_err(|e| format!("Failed to write to execution log: {}", e))?;

    Ok(())
}

// Log to the cronjob errors log
fn log_to_errors(config: &Config, message: &str) -> Result<(), String> {
    let log_path = Path::new(&config.project_dir).join("storage").join("cronjobs").join("errors.log");

    let mut file = OpenOptions::new().create(true).append(true).open(log_path).map_err(|e| format!("Failed to open errors log: {}", e))?;

    let timestamp = Local::now().format("[%Y-%m-%d %H:%M:%S]");
    writeln!(file, "{} {}", timestamp, message).map_err(|e| format!("Failed to write to errors log: {}", e))?;

    Ok(())
}

// Establish database connection using connection string from .env
fn establish_connection(config: &Config) -> Result<PgConnection, String> {
    // Change to the project directory to ensure we pick up the correct .env file
    let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    std::env::set_current_dir(&config.project_dir).map_err(|e| format!("Failed to change to project directory: {}", e))?;

    // Load .env file
    dotenv().ok();

    // Get database URL
    let database_url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL not found in .env file".to_string())?;

    // Restore original directory
    std::env::set_current_dir(current_dir).map_err(|e| format!("Failed to restore directory: {}", e))?;

    // Connect to database
    PgConnection::establish(&database_url).map_err(|e| format!("Error connecting to database: {}", e))
}

// Check if cronjobs table exists
fn check_cronjobs_table(conn: &mut PgConnection) -> Result<bool, String> {
    let results = sql_query("SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'cronjobs') as exists")
        .load::<BoolResult>(conn)
        .map_err(|e| format!("Failed to check if cronjobs table exists: {}", e))?;

    if results.is_empty() {
        Ok(false)
    } else {
        Ok(results[0].exists)
    }
}

// Ensure cronjobs table exists (create if needed)
fn ensure_cronjobs_table(conn: &mut PgConnection) -> Result<(), String> {
    // Check if table exists first
    if !check_cronjobs_table(conn)? {
        // Create table without IF NOT EXISTS since we already checked
        sql_query(
            r#"
            CREATE TABLE cronjobs (
                id SERIAL PRIMARY KEY,
                name VARCHAR NOT NULL UNIQUE,
                timer INT NOT NULL,
                status VARCHAR NOT NULL DEFAULT 'active',
                last_run BIGINT
            );
            
            CREATE INDEX idx_cronjobs_name ON cronjobs(name);
        "#,
        )
        .execute(conn)
        .map_err(|e| format!("Failed to create cronjobs table: {}", e))?;

        // Add example cronjobs (without ON CONFLICT since the table is new)
        sql_query(
            r#"
            INSERT INTO cronjobs (name, timer, status) 
            VALUES 
                ('cleanup_temp_files', 3600, 'active'),
                ('send_digest_emails', 86400, 'active'),
                ('update_search_index', 43200, 'paused');
        "#,
        )
        .execute(conn)
        .map_err(|e| format!("Failed to insert example cronjobs: {}", e))?;
    }

    Ok(())
}

// List all cronjobs with their status
pub fn list_cronjobs(config: &Config) -> Result<(), String> {
    ensure_cronjob_dirs(config).map_err(|e| format!("Failed to create cronjob directories: {}", e))?;

    // Connect to database
    let mut conn = establish_connection(config)?;

    // Ensure cronjobs table exists
    ensure_cronjobs_table(&mut conn)?;

    // Fetch cronjobs
    let jobs = sql_query("SELECT id, name, timer, status, last_run FROM cronjobs ORDER BY id")
        .load::<CronjobInfo>(&mut conn)
        .map_err(|e| format!("Failed to load cronjobs: {}", e))?;

    if jobs.is_empty() {
        println!("No scheduled jobs found.");
        return Ok(());
    }

    // Format output
    println!("╔═════╦════════════════════════╦══════════════╦══════════════╦═══════════════════════╦═══════════════════════╗");
    println!("║ ID  ║ Name                   ║ Interval     ║ Status       ║ Last Run              ║ Next Run              ║");
    println!("╠═════╬════════════════════════╬══════════════╬══════════════╬═══════════════════════╬═══════════════════════╣");

    for job in &jobs {
        let display = CronjobDisplay {
            id: job.id,
            name: job.name.clone(),
            interval: format_duration(job.timer),
            status: job.status.clone(),
            last_run: format_timestamp(job.last_run),
            next_run: calc_next_run(job.last_run, job.timer),
        };

        // Create colorized status while preserving padding
        let status_colorized = match display.status.as_str() {
            "active" => format!("\x1b[32m{}\x1b[0m", display.status),    // Green for active
            "paused" => format!("\x1b[33m{}\x1b[0m", display.status),    // Yellow for paused
            "completed" => format!("\x1b[34m{}\x1b[0m", display.status), // Blue for completed
            "failed" => format!("\x1b[31m{}\x1b[0m", display.status),    // Red for failed
            _ => display.status.clone(),
        };

        // Calculate padding needed for status column
        let status_visible_len = display.status.len();
        let padding_needed = if status_visible_len < 12 { 12 - status_visible_len } else { 0 };
        let status_padding = " ".repeat(padding_needed);

        println!(
            "║ {:3} ║ {:22} ║ {:12} ║ {}{} ║ {:21} ║ {:21} ║",
            display.id, display.name, display.interval, status_colorized, status_padding, display.last_run, display.next_run
        );
    }

    println!("╚═════╩════════════════════════╩══════════════╩══════════════╩═══════════════════════╩═══════════════════════╝");

    Ok(())
}

// Add a new cronjob
pub fn add_cronjob(config: &Config, name: &str, interval: i32) -> Result<(), String> {
    ensure_cronjob_dirs(config).map_err(|e| format!("Failed to create cronjob directories: {}", e))?;

    // Connect to database
    let mut conn = establish_connection(config)?;

    // Ensure cronjobs table exists
    ensure_cronjobs_table(&mut conn)?;

    // Check if job with this name already exists
    let exists_results = sql_query(&format!("SELECT EXISTS (SELECT 1 FROM cronjobs WHERE name = '{}') as exists", name))
        .load::<BoolResult>(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    if !exists_results.is_empty() && exists_results[0].exists {
        return Err(format!("A job with name '{}' already exists", name));
    }

    // Insert new cronjob
    sql_query(&format!("INSERT INTO cronjobs (name, timer, status) VALUES ('{}', {}, 'active')", name, interval))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to add cronjob: {}", e))?;

    // Log action
    log_to_execution(config, &format!("Added new job '{}' with interval of {}", name, format_duration(interval)))?;

    logger::success(&format!("Added new cronjob '{}' with interval of {}", name, format_duration(interval)))?;

    Ok(())
}

// Toggle a cronjob's active status
pub fn toggle_cronjob(config: &Config, id: i32) -> Result<(), String> {
    ensure_cronjob_dirs(config).map_err(|e| format!("Failed to create cronjob directories: {}", e))?;

    // Connect to database
    let mut conn = establish_connection(config)?;

    // Ensure cronjobs table exists
    ensure_cronjobs_table(&mut conn)?;

    // Check if the job exists
    let exists_results = sql_query(&format!("SELECT EXISTS (SELECT 1 FROM cronjobs WHERE id = {}) as exists", id))
        .load::<BoolResult>(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(format!("No job found with ID {}", id));
    }

    // Get current status
    let status_results = sql_query(&format!("SELECT status as result FROM cronjobs WHERE id = {}", id))
        .load::<StringResult>(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    if status_results.is_empty() {
        return Err(format!("Failed to get status for job ID {}", id));
    }

    let current_status = &status_results[0].result;

    // Determine new status
    let new_status = if current_status == "active" { "paused" } else { "active" };

    // Update status
    sql_query(&format!("UPDATE cronjobs SET status = '{}' WHERE id = {}", new_status, id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to update job status: {}", e))?;

    // Get job name for logging
    let name_results = sql_query(&format!("SELECT name as result FROM cronjobs WHERE id = {}", id))
        .load::<StringResult>(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    if name_results.is_empty() {
        return Err(format!("Failed to get name for job ID {}", id));
    }

    let job_name = &name_results[0].result;

    // Log action
    log_to_execution(config, &format!("Job '{}' (ID: {}) status changed from '{}' to '{}'", job_name, id, current_status, new_status))?;

    logger::success(&format!("Job '{}' is now {}", job_name, new_status))?;

    Ok(())
}

// Remove a cronjob
pub fn remove_cronjob(config: &Config, id: i32) -> Result<(), String> {
    ensure_cronjob_dirs(config).map_err(|e| format!("Failed to create cronjob directories: {}", e))?;

    // Connect to database
    let mut conn = establish_connection(config)?;

    // Ensure cronjobs table exists
    ensure_cronjobs_table(&mut conn)?;

    // Check if the job exists
    let exists_results = sql_query(&format!("SELECT EXISTS (SELECT 1 FROM cronjobs WHERE id = {}) as exists", id))
        .load::<BoolResult>(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(format!("No job found with ID {}", id));
    }

    // Get job name for logging
    let name_results = sql_query(&format!("SELECT name as result FROM cronjobs WHERE id = {}", id))
        .load::<StringResult>(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;

    if name_results.is_empty() {
        return Err(format!("Failed to get name for job ID {}", id));
    }

    let job_name = &name_results[0].result;

    // Delete job
    sql_query(&format!("DELETE FROM cronjobs WHERE id = {}", id))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete job: {}", e))?;

    // Log action
    log_to_execution(config, &format!("Removed job '{}' (ID: {})", job_name, id))?;

    logger::success(&format!("Removed cronjob '{}' (ID: {})", job_name, id))?;

    Ok(())
}

