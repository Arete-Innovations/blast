use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::StatusKind;

import_crate_style!(style, "src/transport/leptos/components/status_dot.module.scss");

#[component]
pub fn StatusDot(kind: StatusKind, label: String) -> impl IntoView {
    let kind_attr = kind.as_str();
    view! {
        <span class=style::wrap data-kind=kind_attr>
            <span class=style::dot></span>
            <span class=style::label>{label}</span>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_distinct_str() {
        assert_eq!(StatusKind::Online.as_str(), "online");
        assert_eq!(StatusKind::Offline.as_str(), "offline");
        assert_eq!(StatusKind::Pending.as_str(), "pending");
        assert_eq!(StatusKind::Error.as_str(), "error");
    }
}
