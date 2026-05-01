use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::{StepItem, StepStatus};

import_crate_style!(style, "src/transport/leptos/components/stepper.module.scss");

#[component]
pub fn Stepper(items: Vec<StepItem>) -> impl IntoView {
    let last_idx = items.len().saturating_sub(1);
    view! {
        <ol class=style::wrap>
            {items.into_iter().enumerate().map(|(idx, item)| {
                let status_attr = item.status.as_str();
                let label = item.label;
                let number = format!("{}", idx + 1);
                let show_connector = idx < last_idx;
                let glyph = match item.status {
                    StepStatus::Done => "\u{2713}".to_string(),
                    StepStatus::Error => "\u{2717}".to_string(),
                    StepStatus::Pending => number.clone(),
                    StepStatus::Active => number.clone(),
                };
                view! {
                    <li class=style::item data-status=status_attr>
                        <span class=style::marker aria-hidden="true">{glyph}</span>
                        <span class=style::label>{label}</span>
                        <Show when=move || show_connector fallback=|| ()>
                            <span class=style::connector aria-hidden="true"></span>
                        </Show>
                    </li>
                }
            }).collect_view()}
        </ol>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_status_strings() {
        assert_eq!(StepStatus::Pending.as_str(), "pending");
        assert_eq!(StepStatus::Active.as_str(), "active");
        assert_eq!(StepStatus::Done.as_str(), "done");
        assert_eq!(StepStatus::Error.as_str(), "error");
    }

    #[test]
    fn step_item_constructor() {
        let item = StepItem::new("Init", StepStatus::Active);
        assert_eq!(item.label, "Init");
        assert_eq!(item.status, StepStatus::Active);
    }
}
