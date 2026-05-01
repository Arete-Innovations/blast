use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::PageLayout;

import_crate_style!(style, "src/transport/leptos/components/page_shell.module.scss");

#[component]
pub fn PageShell(layout: PageLayout, children: Children) -> impl IntoView {
    view! {
        <main class=style::shell data-layout=layout.as_str()>
            {children()}
        </main>
    }
}
