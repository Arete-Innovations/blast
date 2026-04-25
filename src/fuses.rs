use crate::configs::Config;
use crate::error::{BlastError, BlastResult};
use crate::logger;
use chrono::Local;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::*;
use diesel::{PgConnection, RunQueryDsl};
use dotenv::dotenv;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;


#[derive(Debug, QueryableByName)]
pub struct FuseInfo {
    #[diesel(sql_type = BigInt)]
    pub id: i64,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Text)]
    pub flow_name: String,
    #[diesel(sql_type = Text)]
    pub schedule_kind: String,
    #[diesel(sql_type = Text)]
    pub schedule_spec: String,
    #[diesel(sql_type = Bool)]
    pub enabled: bool,
    #[diesel(sql_type = Nullable<Text>)]
    pub last_run_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub last_error: Option<String>,
    #[diesel(sql_type = BigInt)]
    pub run_count: i64,
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

fn ensure_fuse_dirs(config: &Config) -> BlastResult<()> {
    let fuse_dir = Path::new(&config.project_dir).join("storage").join("fuses");
    fs::create_dir_all(&fuse_dir)?;

    let execution_log = fuse_dir.join("execution.log");
    let errors_log = fuse_dir.join("errors.log");

    if !execution_log.exists() {
        let mut file = File::create(&execution_log)?;
        writeln!(file, "--- Fuses Execution Log ---")?;
    }

    if !errors_log.exists() {
        let mut file = File::create(&errors_log)?;
        writeln!(file, "--- Fuses Error Log ---")?;
    }

    Ok(())
}

fn log_to_execution(config: &Config, message: &str) -> BlastResult<()> {
    let log_path = Path::new(&config.project_dir)
        .join("storage")
        .join("fuses")
        .join("execution.log");

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

fn check_fuses_table(conn: &mut PgConnection) -> BlastResult<bool> {
    let results = sql_query(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'fuses') as exists",
    )
    .load::<BoolResult>(conn)?;

    if results.is_empty() {
        Ok(false)
    } else {
        Ok(results[0].exists)
    }
}

pub fn list_fuses(config: &Config) -> BlastResult<()> {
    ensure_fuse_dirs(config)?;

    let mut conn = establish_connection(config)?;

    if !check_fuses_table(&mut conn)? {
        println!("Fuses table not found. Create it with a migration using FUSES_MIGRATION_UP.");
        return Ok(());
    }

    let jobs = sql_query(
        "SELECT id, name, flow_name, schedule_kind, schedule_spec, \
         enabled, last_run_status, last_error, run_count \
         FROM fuses ORDER BY id",
    )
    .load::<FuseInfo>(&mut conn)?;

    if jobs.is_empty() {
        println!("No fuses registered.");
        return Ok(());
    }

    println!(
        "╔══════╦══════════════════════════╦══════════════╦══════════╦═══════════╦═══════╦═══════╗"
    );
    println!(
        "║ ID   ║ Name                     ║ Flow         ║ Kind     ║ Spec      ║ On?   ║ Runs  ║"
    );
    println!(
        "╠══════╬══════════════════════════╬══════════════╬══════════╬═══════════╬═══════╬═══════╣"
    );

    for job in &jobs {
        let enabled_str = if job.enabled { "yes" } else { "no" };
        let enabled_colorized = if job.enabled {
            format!("\x1b[32m{}\x1b[0m", enabled_str)
        } else {
            format!("\x1b[33m{}\x1b[0m", enabled_str)
        };
        let enabled_visible_len = enabled_str.len();
        let enabled_padding =
            " ".repeat(if enabled_visible_len < 5 { 5 - enabled_visible_len } else { 0 });

        println!(
            "║ {:4} ║ {:24} ║ {:12} ║ {:8} ║ {:9} ║ {}{} ║ {:5} ║",
            job.id,
            &job.name[..job.name.len().min(24)],
            &job.flow_name[..job.flow_name.len().min(12)],
            &job.schedule_kind[..job.schedule_kind.len().min(8)],
            &job.schedule_spec[..job.schedule_spec.len().min(9)],
            enabled_colorized,
            enabled_padding,
            job.run_count,
        );
    }

    println!(
        "╚══════╩══════════════════════════╩══════════════╩══════════╩═══════════╩═══════╩═══════╝"
    );

    Ok(())
}

pub fn toggle_fuse(config: &Config, name: &str) -> BlastResult<()> {
    ensure_fuse_dirs(config)?;

    let mut conn = establish_connection(config)?;

    if !check_fuses_table(&mut conn)? {
        return Err(BlastError::Fuse("fuses table not found".to_string()));
    }

    let exists_results = sql_query(&format!(
        "SELECT EXISTS (SELECT 1 FROM fuses WHERE name = '{}') as exists",
        name
    ))
    .load::<BoolResult>(&mut conn)?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(BlastError::Fuse(format!("no fuse found with name '{}'", name)));
    }

    let status_results = sql_query(&format!(
        "SELECT enabled::text as result FROM fuses WHERE name = '{}'",
        name
    ))
    .load::<StringResult>(&mut conn)?;

    if status_results.is_empty() {
        return Err(BlastError::Fuse(format!(
            "failed to get enabled flag for fuse '{}'",
            name
        )));
    }

    let currently_enabled =
        status_results[0].result == "true" || status_results[0].result == "t";
    let new_enabled = !currently_enabled;

    sql_query(&format!(
        "UPDATE fuses SET enabled = {}, updated_at = NOW() WHERE name = '{}'",
        new_enabled, name
    ))
    .execute(&mut conn)?;

    let new_state = if new_enabled { "enabled" } else { "disabled" };

    log_to_execution(config, &format!("Fuse '{}' toggled to {}", name, new_state))?;
    logger::success(&format!("Fuse '{}' is now {}", name, new_state))?;

    Ok(())
}

pub fn run_fuse(config: &Config, name: &str) -> BlastResult<()> {
    ensure_fuse_dirs(config)?;

    let mut conn = establish_connection(config)?;

    if !check_fuses_table(&mut conn)? {
        return Err(BlastError::Fuse("fuses table not found".to_string()));
    }

    let exists_results = sql_query(&format!(
        "SELECT EXISTS (SELECT 1 FROM fuses WHERE name = '{}') as exists",
        name
    ))
    .load::<BoolResult>(&mut conn)?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(BlastError::Fuse(format!("no fuse found with name '{}'", name)));
    }

    sql_query(&format!(
        "UPDATE fuses SET next_run_at = NOW(), updated_at = NOW() WHERE name = '{}'",
        name
    ))
    .execute(&mut conn)?;

    log_to_execution(config, &format!("Fuse '{}' queued for immediate run", name))?;
    logger::success(&format!(
        "Fuse '{}' queued for immediate run (next_run_at set to now)",
        name
    ))?;

    Ok(())
}

pub fn logs_fuse(config: &Config, name: &str) -> BlastResult<()> {
    ensure_fuse_dirs(config)?;

    let mut conn = establish_connection(config)?;

    if !check_fuses_table(&mut conn)? {
        return Err(BlastError::Fuse("fuses table not found".to_string()));
    }

    let exists_results = sql_query(&format!(
        "SELECT EXISTS (SELECT 1 FROM fuses WHERE name = '{}') as exists",
        name
    ))
    .load::<BoolResult>(&mut conn)?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(BlastError::Fuse(format!("no fuse found with name '{}'", name)));
    }

    let info = sql_query(&format!(
        "SELECT id, name, flow_name, schedule_kind, schedule_spec, \
         enabled, last_run_status, last_error, run_count \
         FROM fuses WHERE name = '{}'",
        name
    ))
    .load::<FuseInfo>(&mut conn)?;

    if info.is_empty() {
        return Err(BlastError::Fuse(format!("failed to fetch fuse '{}'", name)));
    }

    let fuse = &info[0];
    println!("Fuse: {}", fuse.name);
    println!("  flow_name:       {}", fuse.flow_name);
    println!("  schedule_kind:   {}", fuse.schedule_kind);
    println!("  schedule_spec:   {}", fuse.schedule_spec);
    println!("  enabled:         {}", fuse.enabled);
    let status_display = match fuse.last_run_status.as_deref() {
        Some(s) => s.to_string(),
        None => "-".to_string(),
    };
    let error_display = match fuse.last_error.as_deref() {
        Some(s) => s.to_string(),
        None => "-".to_string(),
    };
    println!("  last_run_status: {}", status_display);
    println!("  last_error:      {}", error_display);
    println!("  run_count:       {}", fuse.run_count);

    let log_path = Path::new(&config.project_dir)
        .join("storage")
        .join("fuses")
        .join("execution.log");

    if log_path.exists() {
        let contents = fs::read_to_string(&log_path)?;
        let relevant: Vec<&str> = contents
            .lines()
            .filter(|line| line.contains(&format!("'{}'", name)))
            .collect();

        if relevant.is_empty() {
            println!("\nNo log entries found for fuse '{}'.", name);
        } else {
            println!("\nRecent log entries:");
            let start = relevant.len().saturating_sub(20);
            for line in &relevant[start..] {
                println!("  {}", line);
            }
        }
    }

    Ok(())
}

pub fn remove_fuse(config: &Config, id: i32) -> BlastResult<()> {
    ensure_fuse_dirs(config)?;

    let mut conn = establish_connection(config)?;

    if !check_fuses_table(&mut conn)? {
        return Err(BlastError::Fuse("fuses table not found".to_string()));
    }

    let exists_results = sql_query(&format!(
        "SELECT EXISTS (SELECT 1 FROM fuses WHERE id = {}) as exists",
        id
    ))
    .load::<BoolResult>(&mut conn)?;

    if exists_results.is_empty() || !exists_results[0].exists {
        return Err(BlastError::Fuse(format!("no fuse found with ID {}", id)));
    }

    let name_results =
        sql_query(&format!("SELECT name as result FROM fuses WHERE id = {}", id))
            .load::<StringResult>(&mut conn)?;

    if name_results.is_empty() {
        return Err(BlastError::Fuse(format!("failed to get name for fuse ID {}", id)));
    }

    let fuse_name = &name_results[0].result;

    sql_query(&format!("DELETE FROM fuses WHERE id = {}", id)).execute(&mut conn)?;

    log_to_execution(config, &format!("Removed fuse '{}' (ID: {})", fuse_name, id))?;
    logger::success(&format!("Removed fuse '{}' (ID: {})", fuse_name, id))?;

    Ok(())
}
