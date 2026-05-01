use leptos::prelude::*;

use crate::meltdown::MeltDown;

#[component]
pub fn ErrorBanner(error: MeltDown) -> impl IntoView {
    let msg = error.to_string();
    view! {
        <div class="error-banner">
            <strong>"Error: "</strong>
            <span>{msg}</span>
        </div>
    }
}
