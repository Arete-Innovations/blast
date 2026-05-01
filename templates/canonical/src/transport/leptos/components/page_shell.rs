use leptos::prelude::*;

use crate::structs::leptos::PageLayout;

#[component]
pub fn PageShell(layout: PageLayout, children: Children) -> impl IntoView {
    view! {
        <main class=layout.class()>
            {children()}
        </main>
    }
}
