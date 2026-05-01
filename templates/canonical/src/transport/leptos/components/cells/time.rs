use chrono::{DateTime, Timelike, Utc};
use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/time.module.scss");

#[component]
pub fn TimeCell(value: DateTime<Utc>) -> impl IntoView {
    let iso = value.to_rfc3339();
    let display = format!("{:02}:{:02}:{:02}", value.hour(), value.minute(), value.second());
    view! {
        <time class=style::time datetime=iso>{display}</time>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn time_format() {
        let dt = Utc.with_ymd_and_hms(2026, 3, 7, 14, 5, 9).unwrap();
        let s = format!("{:02}:{:02}:{:02}", dt.hour(), dt.minute(), dt.second());
        assert_eq!(s, "14:05:09");
    }
}
