use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::database::schema::fuses;

#[derive(Insertable)]
#[diesel(table_name = fuses)]
pub(crate) struct NewFuseRow<'a> {
    pub name: &'a str,
    pub flow_name: &'a str,
    pub schedule_kind: &'a str,
    pub schedule_spec: &'a str,
    pub next_run_at: DateTime<Utc>,
}
