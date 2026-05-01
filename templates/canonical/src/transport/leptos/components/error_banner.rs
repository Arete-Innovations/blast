use leptos::prelude::*;
use stylance::import_crate_style;

use crate::meltdown::MeltDown;

import_crate_style!(style, "src/transport/leptos/components/error_banner.module.scss");

#[component]
pub fn ErrorBanner(error: MeltDown) -> impl IntoView {
    let msg = error.to_string();
    view! {
        <div class=style::banner>
            <strong>"Error: "</strong>
            <span>{msg}</span>
        </div>
    }
}
