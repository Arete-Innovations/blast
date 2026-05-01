use leptos::prelude::*;

use crate::structs::leptos::RouteName;
use crate::transport::leptos::components::{PageLayout, PageShell};

#[component]
pub fn WelcomePage() -> impl IntoView {
    view! {
        <PageShell layout=PageLayout::Cards>
            <h1>"Catablast"</h1>
            <p>"Strongly-typed Rust web-app stack."</p>
            <p>
                <a href=RouteName::Login.path()>"Login"</a>
                " | "
                <a href=RouteName::Register.path()>"Register"</a>
            </p>
        </PageShell>
    }
}
