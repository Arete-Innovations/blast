use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/empty.module.scss");

#[component]
pub fn EmptyCell() -> impl IntoView {
    view! {
        <span class=style::empty>"\u{2014}"</span>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_cell_exists() {
        assert!(true);
    }
}
