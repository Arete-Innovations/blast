use leptos::prelude::*;

use crate::transport::leptos::components::{PageLayout, PageShell};

#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <PageShell layout=PageLayout::Cards>
            <h1>"404 — not found"</h1>
            <p><a href="/">"Back home"</a></p>
        </PageShell>
    }
}
