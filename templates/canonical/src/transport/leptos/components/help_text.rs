use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/help_text.module.scss");

#[component]
pub fn HelpText(children: Children) -> impl IntoView {
    view! {
        <span class=style::hint>{children()}</span>
    }
}
