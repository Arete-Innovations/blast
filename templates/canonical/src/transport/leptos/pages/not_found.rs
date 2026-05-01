use leptos::prelude::*;

use crate::structs::leptos::RouteName;
use crate::transport::leptos::components::{PageLayout, PageShell};

#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <PageShell layout=PageLayout::Cards>
            <h1>"404 — not found"</h1>
            <p><a href=RouteName::Welcome.path()>"Back home"</a></p>
        </PageShell>
    }
}
