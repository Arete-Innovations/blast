use leptos::prelude::*;

use crate::structs::leptos::RouteName;
use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, PageLayout, PageShell};

#[component]
pub fn DashboardPage() -> impl IntoView {
    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Cards>
                <h1>"Dashboard"</h1>
                <p>"Authenticated landing page."</p>
                <p><a href={RouteName::Logout.path().to_string()}>"Sign out"</a></p>
            </PageShell>
        </AuthGuard>
    }
}
