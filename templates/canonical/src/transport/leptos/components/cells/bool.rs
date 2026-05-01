use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::BoolVariant;

import_crate_style!(style, "src/transport/leptos/components/cells/bool.module.scss");

#[component]
pub fn BoolCell(
    value: bool,
    #[prop(default = BoolVariant::Check)] variant: BoolVariant,
) -> impl IntoView {
    match variant {
        BoolVariant::Check => {
            let glyph = if value { "\u{2713}" } else { "\u{2717}" };
            let cls = if value {
                format!("{} {}", style::check, style::true_val)
            } else {
                format!("{} {}", style::check, style::false_val)
            };
            view! {
                <span class=cls aria-label=if value { "true" } else { "false" }>{glyph}</span>
            }
            .into_any()
        }
        BoolVariant::YesNo => {
            let text = if value { "Yes" } else { "No" };
            let cls = if value {
                format!("{} {}", style::yesno, style::true_val)
            } else {
                format!("{} {}", style::yesno, style::false_val)
            };
            view! {
                <span class=cls>{text}</span>
            }
            .into_any()
        }
        BoolVariant::Badge => {
            let text = if value { "true" } else { "false" };
            let cls = if value {
                format!("{} {}", style::badge, style::badge_true)
            } else {
                format!("{} {}", style::badge, style::badge_false)
            };
            view! {
                <span class=cls>{text}</span>
            }
            .into_any()
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bool_variants_exist() {
        use crate::structs::leptos::BoolVariant;
        let _check = BoolVariant::Check;
        let _yesno = BoolVariant::YesNo;
        let _badge = BoolVariant::Badge;
    }
}
