use leptos::prelude::*;

use crate::structs::leptos::Theme;
use crate::transport::leptos::signals::theme::use_theme;

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
        <button type="button" on:click=on_click class="dark-mode-toggle">
            {move || theme.get().as_str()}
        </button>
    }
}
