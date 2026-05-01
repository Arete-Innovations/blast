use std::time::Duration;

use chrono::{DateTime, NaiveTime, Utc};

use crate::{cata_log, structs::fuses::schedule::Schedule};

impl Schedule {
    pub fn every(d: Duration) -> Self {
        Schedule::Every(d)
    }

    pub fn cron(expr: impl Into<String>) -> Self {
        Schedule::Cron(expr.into())
    }

    pub fn at(time: NaiveTime) -> Self {
        Schedule::DailyAt(time)
    }

    pub fn spec_string(&self) -> String {
        match self {
            Schedule::Every(d) => format!("every:{}s", d.as_secs()),
            Schedule::Cron(expr) => format!("cron:{}", expr),
            Schedule::DailyAt(t) => format!("daily_at:{}", t.format("%H:%M:%S")),
        }
    }

    pub fn kind_string(&self) -> &'static str {
        match self {
            Schedule::Every(_) => "interval",
            Schedule::Cron(_) => "cron",
            Schedule::DailyAt(_) => "daily_at",
        }
    }
}

pub fn schedule_from_row(kind: &str, spec: &str) -> Option<Schedule> {
    match kind {
        "interval" => {
            let stripped = spec.strip_prefix("every:")?.strip_suffix('s')?;
            let secs: u64 = match stripped.parse() {
                Ok(n) => n,
                Err(e) => {
                    cata_log!(Debug, format!("schedule_from_row interval parse '{}': {}", stripped, e));
                    return None;
                }
            };
            Some(Schedule::Every(Duration::from_secs(secs)))
        }
        "cron" => {
            let expr = spec.strip_prefix("cron:")?;
            Some(Schedule::Cron(expr.to_string()))
        }
        "daily_at" => {
            let raw = spec.strip_prefix("daily_at:")?;
            let t = match NaiveTime::parse_from_str(raw, "%H:%M:%S") {
                Ok(time) => time,
                Err(e) => {
                    cata_log!(Debug, format!("schedule_from_row daily_at parse '{}': {}", raw, e));
                    return None;
                }
            };
            Some(Schedule::DailyAt(t))
        }
        other => {
            cata_log!(Debug, format!("schedule_from_row: unknown kind '{}'", other));
            None
        }
    }
}

impl Schedule {
    pub fn next_run_after(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Schedule::Every(d) => {
                let delta = match chrono::Duration::from_std(*d) {
                    Ok(dur) => dur,
                    Err(e) => {
                        cata_log!(Warning, format!("Schedule::Every duration overflow: {}", e));
                        chrono::Duration::seconds(i64::MAX)
                    }
                };
                let Some(result) = now.checked_add_signed(delta) else {
                    return now;
                };
                result
            }
            Schedule::Cron(expr) => {
                let parsed: cron::Schedule = match expr.parse() {
                    Ok(p) => p,
                    Err(e) => panic!("invalid cron expression {:?}: {}", expr, e),
                };
                let Some(next) = parsed.after(&now).next() else {
                    return now + chrono::Duration::seconds(60);
                };
                next
            }
            Schedule::DailyAt(t) => {
                let today = now.date_naive().and_time(*t).and_utc();
                if today > now {
                    today
                } else {
                    today + chrono::Duration::days(1)
                }
            }
        }
    }
}
