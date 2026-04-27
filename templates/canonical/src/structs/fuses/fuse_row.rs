use chrono::{DateTime, Utc};
use diesel::prelude::*;

#[derive(Debug, Clone, Queryable)]
pub(crate) struct FuseRow {
    pub id: i64,
    pub name: String,
    pub flow_name: String,
    pub schedule_kind: String,
    pub schedule_spec: String,
    pub enabled: bool,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<String>,
    pub last_error: Option<String>,
    pub next_run_at: DateTime<Utc>,
    pub run_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
