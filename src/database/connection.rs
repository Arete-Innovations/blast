use crate::error::BlastError;
use crate::error::BlastResult;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use std::fs;
use std::process::Command;

pub fn establish_connection() -> BlastResult<PgConnection> {
    let env_content = fs::read_to_string(".env").map_err(|e| {
        BlastError::NotFound(format!(
            "Could not read .env file: {}. Make sure it exists in the project root.",
            e
        ))
    })?;

    let mut database_url: Option<&str> = None;
    for line in env_content.lines() {
        if line.starts_with("DATABASE_URL=") && !line.contains("_DATABASE_URL") {
            database_url = line.strip_prefix("DATABASE_URL=").map(|s| s.trim().trim_matches('"'));
            break;
        }
    }

    let postgres_available = match Command::new("which").arg("psql").output() {
        Ok(output) => output.status.success(),
        Err(e) => {
            drop(e);
            false
        }
    };

    let database_url = match database_url {
        Some(url) => url,
        None => {
            let suggestion = if postgres_available {
                "DATABASE_URL environment variable not found in .env file. Make sure you have a .env file with DATABASE_URL=postgres://username:password@localhost/dbname"
            } else {
                "DATABASE_URL environment variable not found in .env file and PostgreSQL might not be installed. \
                Please install PostgreSQL and create a .env file with DATABASE_URL=postgres://username:password@localhost/dbname"
            };
            return Err(BlastError::NotFound(suggestion.to_string()));
        }
    };

    let masked_url = mask_url(database_url);
    if let Err(e) = crate::logger::info(&format!("Connecting to database: {}", masked_url)) {
        drop(e);
    }

    PgConnection::establish(database_url).map_err(|e| {
        let service_running = match Command::new("pg_isready").args(["-h", "localhost"]).output() {
            Ok(output) => output.status.success(),
            Err(e) => {
                drop(e);
                false
            }
        };

        let error_message = format!("Could not connect to database via `{}`: {}", masked_url, e);
        let suggestion = if !service_running {
            format!(
                "{}. PostgreSQL service appears to be down. Try starting it with: sudo service postgresql start",
                error_message
            )
        } else {
            format!(
                "{}. PostgreSQL is running but connection failed. Check your credentials and database existence",
                error_message
            )
        };
        BlastError::Project(suggestion)
    })
}

pub fn mask_url(url: &str) -> String {
    if url.contains("://") {
        let parts: Vec<&str> = url.splitn(2, "://").collect();
        if parts.len() == 2 {
            return format!("{}://<masked>", parts[0]);
        }
    }
    "<masked>".to_string()
}
