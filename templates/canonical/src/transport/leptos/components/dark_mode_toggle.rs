use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::Theme;
use crate::transport::leptos::signals::theme::use_theme;

import_crate_style!(style, "src/transport/leptos/components/dark_mode_toggle.module.scss");

#[component]
pub fn DarkModeToggle() -> impl IntoView {
    let theme = use_theme();
    let on_click = move |_| {
        let next = match theme.get() {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::System,
            Theme::System => Theme::Light,
        };
        theme.set(next);
    };
    view! {
        <button type="button" on:click=on_click class=style::toggle>
            {move || theme.get().as_str()}
        </button>
    }
}
