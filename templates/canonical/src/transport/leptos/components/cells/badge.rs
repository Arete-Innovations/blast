use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::BadgeColor;

import_crate_style!(style, "src/transport/leptos/components/cells/badge.module.scss");

#[component]
pub fn BadgeCell(
    text: String,
    #[prop(default = BadgeColor::Default)] color: BadgeColor,
) -> impl IntoView {
    let cls = match color {
        BadgeColor::Default => style::badge.to_string(),
        BadgeColor::Success => format!("{} {}", style::badge, style::success),
        BadgeColor::Warning => format!("{} {}", style::badge, style::warning),
        BadgeColor::Danger => format!("{} {}", style::badge, style::danger),
        BadgeColor::Info => format!("{} {}", style::badge, style::info),
    };
    view! {
        <span class=cls>{text}</span>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn badge_colors_exist() {
        use crate::structs::leptos::BadgeColor;
        let _default = BadgeColor::Default;
        let _success = BadgeColor::Success;
        let _warning = BadgeColor::Warning;
        let _danger = BadgeColor::Danger;
        let _info = BadgeColor::Info;
    }
}
