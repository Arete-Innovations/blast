use leptos::prelude::*;

use crate::transport::leptos::components::{PageLayout, PageShell};

#[component]
pub fn WelcomePage() -> impl IntoView {
    view! {
        <PageShell layout=PageLayout::Cards>
            <h1>"Catablast"</h1>
            <p>"Strongly-typed Rust web-app stack."</p>
            <p>
                <a href="/login">"Login"</a>
                " | "
                <a href="/register">"Register"</a>
            </p>
        </PageShell>
    }
}
