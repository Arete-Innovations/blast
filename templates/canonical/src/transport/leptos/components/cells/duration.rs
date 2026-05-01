use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/duration.module.scss");

fn humanize_ms(ms: i64) -> String {
    if ms < 0 {
        return format!("-{}", humanize_ms(-ms));
    }
    let total_secs = ms / 1000;
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        if hours > 0 {
            format!("{}d {}h", days, hours)
        } else {
            format!("{}d", days)
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    } else if minutes > 0 {
        if secs > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}m", minutes)
        }
    } else {
        format!("{}s", secs)
    }
}

#[component]
pub fn DurationCell(ms: i64) -> impl IntoView {
    let text = humanize_ms(ms);
    view! {
        <span class=style::duration>{text}</span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_only() {
        assert_eq!(humanize_ms(45_000), "45s");
    }

    #[test]
    fn minutes_and_seconds() {
        assert_eq!(humanize_ms(135_000), "2m 15s");
    }

    #[test]
    fn hours_and_minutes() {
        assert_eq!(humanize_ms(8_100_000), "2h 15m");
    }

    #[test]
    fn days_and_hours() {
        assert_eq!(humanize_ms(90_000_000), "1d 1h");
    }

    #[test]
    fn days_only() {
        assert_eq!(humanize_ms(3 * 86400 * 1000), "3d");
    }

    #[test]
    fn zero_ms() {
        assert_eq!(humanize_ms(0), "0s");
    }
}
