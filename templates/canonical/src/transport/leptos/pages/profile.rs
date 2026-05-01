use leptos::prelude::*;

use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, PageLayout, PageShell};

#[component]
pub fn ProfilePage() -> impl IntoView {
    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Cards>
                <h1>"Profile"</h1>
                <p>"User profile page."</p>
            </PageShell>
        </AuthGuard>
    }
}
