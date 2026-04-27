
use chrono::{DateTime, NaiveTime, Utc};
use std::time::Duration;

use crate::cata_log;
use crate::structs::fuses::schedule::Schedule;

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
                let today = now
                    .date_naive()
                    .and_time(*t)
                    .and_utc();
                if today > now {
                    today
                } else {
                    today + chrono::Duration::days(1)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};

    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(h, mi, s)
                .unwrap(),
        )
    }

    #[test]
    fn every_adds_duration() {
        let now = at(2026, 4, 25, 12, 0, 0);
        let next = Schedule::Every(Duration::from_secs(300)).next_run_after(now);
        assert_eq!(next, at(2026, 4, 25, 12, 5, 0));
    }

    #[test]
    fn cron_picks_next_match() {
        let now = at(2026, 4, 25, 12, 0, 0);
        let next = Schedule::Cron("0 0 2 * * *".to_string()).next_run_after(now);
        assert_eq!(next, at(2026, 4, 26, 2, 0, 0));
    }

    #[test]
    fn daily_at_picks_today_or_tomorrow() {
        let morning = at(2026, 4, 25, 8, 0, 0);
        let target = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let next = Schedule::DailyAt(target).next_run_after(morning);
        assert_eq!(next, at(2026, 4, 25, 9, 0, 0));

        let evening = at(2026, 4, 25, 21, 0, 0);
        let next = Schedule::DailyAt(target).next_run_after(evening);
        assert_eq!(next, at(2026, 4, 26, 9, 0, 0));
    }
}
