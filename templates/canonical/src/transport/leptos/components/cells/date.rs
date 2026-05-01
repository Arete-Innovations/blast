use chrono::{DateTime, Datelike, Timelike, Utc};
use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::DateFormat;

import_crate_style!(style, "src/transport/leptos/components/cells/date.module.scss");

const MONTH_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const DOW_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn format_date(dt: &DateTime<Utc>, fmt: DateFormat) -> String {
    match fmt {
        DateFormat::Iso => dt.to_rfc3339(),
        DateFormat::Short => format!("{:04}-{:02}-{:02}", dt.year(), dt.month(), dt.day()),
        DateFormat::Long => {
            let dow = DOW_SHORT[dt.weekday().num_days_from_sunday() as usize];
            let mon = MONTH_SHORT[dt.month0() as usize];
            format!("{}, {} {} {}", dow, mon, dt.day(), dt.year())
        }
        DateFormat::Time => format!("{:02}:{:02}", dt.hour(), dt.minute()),
    }
}

#[component]
pub fn DateCell(
    value: DateTime<Utc>,
    #[prop(default = DateFormat::Short)] format: DateFormat,
) -> impl IntoView {
    let iso = value.to_rfc3339();
    let display = format_date(&value, format);
    view! {
        <time class=style::date datetime=iso>{display}</time>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 15, 9, 30, 0).unwrap()
    }

    #[test]
    fn short_format() {
        assert_eq!(format_date(&sample(), DateFormat::Short), "2026-01-15");
    }

    #[test]
    fn long_format() {
        assert_eq!(format_date(&sample(), DateFormat::Long), "Thu, Jan 15 2026");
    }

    #[test]
    fn time_format() {
        assert_eq!(format_date(&sample(), DateFormat::Time), "09:30");
    }
}
