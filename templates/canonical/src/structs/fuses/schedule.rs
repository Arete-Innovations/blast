use std::time::Duration;

use chrono::NaiveTime;

#[derive(Debug, Clone)]
pub enum Schedule {
    Every(Duration),
    Cron(String),
    DailyAt(NaiveTime),
}
