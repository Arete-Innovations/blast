use std::time::Duration;

use canonical::{structs::fuses::schedule::Schedule, transport::fuses::schedule::schedule_from_row};
use chrono::NaiveTime;

#[test]
fn round_trip_interval() {
    let s = Schedule::Every(Duration::from_secs(120));
    let parsed = schedule_from_row(s.kind_string(), &s.spec_string()).expect("interval round-trips");
    match parsed {
        Schedule::Every(d) => assert_eq!(d.as_secs(), 120),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn round_trip_cron() {
    let s = Schedule::Cron("0 2 * * *".to_string());
    let parsed = schedule_from_row(s.kind_string(), &s.spec_string()).expect("cron round-trips");
    match parsed {
        Schedule::Cron(expr) => assert_eq!(expr, "0 2 * * *"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn round_trip_daily_at() {
    let s = Schedule::DailyAt(NaiveTime::from_hms_opt(9, 0, 0).expect("9am"));
    let parsed = schedule_from_row(s.kind_string(), &s.spec_string()).expect("daily_at round-trips");
    match parsed {
        Schedule::DailyAt(t) => assert_eq!(t, NaiveTime::from_hms_opt(9, 0, 0).expect("9am")),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn unknown_kind_returns_none() {
    assert!(schedule_from_row("unknown_kind", "anything").is_none());
}

#[test]
fn interval_missing_prefix_returns_none() {
    assert!(schedule_from_row("interval", "30s").is_none(), "must require 'every:' prefix");
}

#[test]
fn interval_missing_suffix_returns_none() {
    assert!(schedule_from_row("interval", "every:30").is_none(), "must require 's' suffix");
}

#[test]
fn interval_non_numeric_returns_none() {
    assert!(schedule_from_row("interval", "every:abcs").is_none(), "non-numeric body fails");
}

#[test]
fn cron_missing_prefix_returns_none() {
    assert!(schedule_from_row("cron", "0 2 * * *").is_none(), "must require 'cron:' prefix");
}

#[test]
fn daily_at_missing_prefix_returns_none() {
    assert!(schedule_from_row("daily_at", "09:00:00").is_none(), "must require 'daily_at:' prefix");
}

#[test]
fn daily_at_malformed_time_returns_none() {
    assert!(schedule_from_row("daily_at", "daily_at:25:99:99").is_none(), "out-of-range time fails");
}

#[test]
fn empty_strings_return_none() {
    assert!(schedule_from_row("", "").is_none());
}
