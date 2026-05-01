use chrono::{DateTime, Utc};
use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/relative_date.module.scss");

fn relative_label(value: &DateTime<Utc>, now: &DateTime<Utc>) -> String {
    let diff = now.signed_duration_since(*value);
    let secs = diff.num_seconds();
    let future = secs < 0;
    let abs = secs.unsigned_abs();

    let (n, unit) = if abs < 60 {
        (abs, "second")
    } else if abs < 3600 {
        (abs / 60, "minute")
    } else if abs < 86400 {
        (abs / 3600, "hour")
    } else if abs < 86400 * 30 {
        (abs / 86400, "day")
    } else if abs < 86400 * 365 {
        (abs / (86400 * 30), "month")
    } else {
        (abs / (86400 * 365), "year")
    };

    let plural = if n == 1 { "" } else { "s" };
    if future {
        format!("in {} {}{}", n, unit, plural)
    } else {
        format!("{} {}{} ago", n, unit, plural)
    }
}

#[component]
pub fn RelativeDateCell(value: DateTime<Utc>) -> impl IntoView {
    let iso = value.to_rfc3339();
    let label = relative_label(&value, &Utc::now());
    view! {
        <time class=style::relative datetime=iso.clone() title=iso>{label}</time>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now_fixed() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap()
    }

    #[test]
    fn minutes_ago() {
        let value = Utc.with_ymd_and_hms(2026, 6, 1, 11, 55, 0).unwrap();
        assert_eq!(relative_label(&value, &now_fixed()), "5 minutes ago");
    }

    #[test]
    fn in_future() {
        let value = Utc.with_ymd_and_hms(2026, 6, 1, 13, 0, 0).unwrap();
        assert_eq!(relative_label(&value, &now_fixed()), "in 1 hour");
    }

    #[test]
    fn just_now() {
        let value = now_fixed();
        assert_eq!(relative_label(&value, &now_fixed()), "0 seconds ago");
    }

    #[test]
    fn days_ago() {
        let value = Utc.with_ymd_and_hms(2026, 5, 29, 12, 0, 0).unwrap();
        assert_eq!(relative_label(&value, &now_fixed()), "3 days ago");
    }
}
