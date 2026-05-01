use std::fmt::Display;

use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/enum_cell.module.scss");

#[component]
pub fn EnumCell<E>(
    value: E,
    color: fn(&E) -> &'static str,
) -> impl IntoView
where
    E: Display + Clone + Send + Sync + 'static,
{
    let label = value.to_string();
    let suffix = color(&value);
    let cls = format!("{} {}", style::pill, style::colored);
    view! {
        <span
            class=cls
            data-variant=suffix
        >
            {label}
        </span>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn enum_cell_exists() {
        assert!(true);
    }
}
