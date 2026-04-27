use std::time::Duration;

use canonical::structs::fuses::schedule::Schedule;
use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};

fn at(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.from_utc_datetime(&NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, mi, s).unwrap())
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
