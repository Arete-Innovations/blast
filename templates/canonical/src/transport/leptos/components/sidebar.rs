use leptos::prelude::*;

use crate::transport::leptos::components::generated::nav::AppNav;

#[component]
pub fn AppSidebar() -> impl IntoView {
    view! {
        <aside class="app-sidebar">
            <AppNav/>
        </aside>
    }
}
