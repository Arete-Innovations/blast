
use chrono::NaiveTime;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Schedule {
    Every(Duration),
    Cron(String),
    DailyAt(NaiveTime),
}
