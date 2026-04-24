use crate::configs::Config;
use crate::error::{BlastError, BlastResult};
use crate::logger;
use chrono::{Local, TimeZone, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::*;
use diesel::{PgConnection, RunQueryDsl};
use dotenv::dotenv;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

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

#[derive(Debug, QueryableByName)]
pub struct BoolResult {
    #[diesel(sql_type = Bool)]
    pub exists: bool,
}

#[derive(Debug, QueryableByName)]
pub struct StringResult {
    #[diesel(sql_type = Text)]
    pub result: String,
}

pub struct CronjobDisplay {
    pub id: i32,
    pub name: String,
    pub interval: String,
    pub status: String,
    pub last_run: String,
    pub next_run: String,
}

fn ensure_cronjob_dirs(config: &Config) -> BlastResult<()> {
    let cronjob_dir = Path::new(&config.project_dir).join("storage").join("cronjobs");
    fs::create_dir_all(&cronjob_dir)?;

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

fn format_timestamp(timestamp: Option<i64>) -> String {
    match timestamp {
        Some(ts) => match Local.timestamp_opt(ts, 0).single() {
            Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => "Invalid timestamp".to_string(),
        },
        None => "Never".to_string(),
    }
}

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

fn log_to_execution(config: &Config, message: &str) -> BlastResult<()> {
    let log_path = Path::new(&config.project_dir).join("storage").join("cronjobs").join("execution.log");

    let mut file = OpenOptions::new().create(true).append(true).open(log_path)?;

    let timestamp = Local::now().format("[%Y-%m-%d %H:%M:%S]");
    writeln!(file, "{} {}", timestamp, message)?;

    Ok(())
}

fn establish_connection(config: &Config) -> BlastResult<PgConnection> {
    let current_dir = std::env::current_dir()?;
    std::env::set_current_dir(&config.project_dir)?;

    if let Err(e) = dotenv() {
        drop(e);
    }

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_e| BlastError::Config("DATABASE_URL not found in .env".to_string()))?;

    std::env::set_current_dir(current_dir)?;

    Ok(PgConnection::establish(&database_url)?)
}

fn check_cronjobs_table(conn: &mut PgConnection) -> BlastResult<bool> {
    let results = sql_query("SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'cronjobs') as exists")
        .load::<BoolResult>(conn)?;

    if results.is_empty() {
        Ok(false)
    } else {
        Ok(results[0].exists)
    }
}

fn ensure_cronjobs_table(conn: &mut PgConnection) -> BlastResult<()> {
    if !check_cronjobs_table(conn)? {
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
        .execute(conn)?;

        sql_query(
            r#"
            INSERT INTO cronjobs (name, timer, status)
            VALUES
                ('cleanup_temp_files', 3600, 'active'),
                ('send_digest_emails', 86400, 'active'),
                ('update_search_index', 43200, 'paused');
        "#,
        )
        .execute(conn)?;
    }

    Ok(())
}

pub fn list_cronjobs(config: &Config) -> BlastResult<()> {
    ensure_cronjob_dirs(config)?;

    let mut conn = establish_connection(config)?;

    ensure_cronjobs_table(&mut conn)?;

    let jobs = sql_query("SELECT id, name, timer, status, last_run FROM cronjobs ORDER BY id")
        .load::<CronjobInfo>(&mut conn)?;

    if jobs.is_empty() {
        println!("No scheduled jobs found.");
        return Ok(());
    }

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

        let status_colorized = match display.status.as_str() {
            "active" => format!("\x1b[32m{}\x1b[0m", display.status),
            "paused" => format!("\x1b[33m{}\x1b[0m", display.status),
            "completed" => format!("\x1b[34m{}\x1b[0m", display.status),
            "failed" => format!("\x1b[31m{}\x1b[0m", display.status),
            other => other.to_string(),
        };

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

pub fn add_cronjob(config: &Config, name: &str, interval: i32) -> BlastResult<()> {
    ensure_cronjob_dirs(config)?;

    let mut conn = establish_connection(config)?;

    ensure_cronjobs_table(&mut conn)?;

    let exists_results = sql_query(&format!("SELECT EXISTS (SELECT 1 FROM cronjobs WHERE name = '{}') as exists", name))
        .load::<BoolResult>(&mut conn)?;

    if !exists_results.is_empty() && exists_results[0].exists {
        return Err(BlastError::Cronjob(format!("a job with name '{}' already exists", name)));
    }

    sql_query(&format!("INSERT INTO cronjobs (name, timer, status) VALUES ('{}', {}, 'active')", name, interval))
        .execute(&mut conn)?;

    log_to_execution(config, &format!("Added new job '{}' with interval of {}", name, format_duration(interval)))?;

    logger::success(&format!("Added new cronjob '{}' with interval of {}", name, format_duration(interval)))?;

    Ok(())
}

pub fn toggle_cronjob(config: &Config, id: i32) -> BlastResult<()> {
    ensure_cronjob_dirs(config)?;

    let mut conn = establish_connection(config)?;

    ensure_cronjobs_table(&mut conn)?;

    let exists_results = sql_query(&format!("SELECT EXISTS (SELECT 1 FROM cronjobs WHERE id = {}) as exists", id))
        .load::<BoolResult>(&mut conn)?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(BlastError::Cronjob(format!("no job found with ID {}", id)));
    }

    let status_results = sql_query(&format!("SELECT status as result FROM cronjobs WHERE id = {}", id))
        .load::<StringResult>(&mut conn)?;

    if status_results.is_empty() {
        return Err(BlastError::Cronjob(format!("failed to get status for job ID {}", id)));
    }

    let current_status = &status_results[0].result;

    let new_status = if current_status == "active" { "paused" } else { "active" };

    sql_query(&format!("UPDATE cronjobs SET status = '{}' WHERE id = {}", new_status, id))
        .execute(&mut conn)?;

    let name_results = sql_query(&format!("SELECT name as result FROM cronjobs WHERE id = {}", id))
        .load::<StringResult>(&mut conn)?;

    if name_results.is_empty() {
        return Err(BlastError::Cronjob(format!("failed to get name for job ID {}", id)));
    }

    let job_name = &name_results[0].result;

    log_to_execution(config, &format!("Job '{}' (ID: {}) status changed from '{}' to '{}'", job_name, id, current_status, new_status))?;

    logger::success(&format!("Job '{}' is now {}", job_name, new_status))?;

    Ok(())
}

pub fn remove_cronjob(config: &Config, id: i32) -> BlastResult<()> {
    ensure_cronjob_dirs(config)?;

    let mut conn = establish_connection(config)?;

    ensure_cronjobs_table(&mut conn)?;

    let exists_results = sql_query(&format!("SELECT EXISTS (SELECT 1 FROM cronjobs WHERE id = {}) as exists", id))
        .load::<BoolResult>(&mut conn)?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(BlastError::Cronjob(format!("no job found with ID {}", id)));
    }

    let name_results = sql_query(&format!("SELECT name as result FROM cronjobs WHERE id = {}", id))
        .load::<StringResult>(&mut conn)?;

    if name_results.is_empty() {
        return Err(BlastError::Cronjob(format!("failed to get name for job ID {}", id)));
    }

    let job_name = &name_results[0].result;

    sql_query(&format!("DELETE FROM cronjobs WHERE id = {}", id))
        .execute(&mut conn)?;

    log_to_execution(config, &format!("Removed job '{}' (ID: {})", job_name, id))?;

    logger::success(&format!("Removed cronjob '{}' (ID: {})", job_name, id))?;

    Ok(())
}
