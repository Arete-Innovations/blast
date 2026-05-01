use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::structs::leptos::RouteName;
use crate::transport::leptos::components::{AuthGuardMode, AuthGuard, ErrorBanner, PageLayout, PageShell};
use crate::transport::leptos::data::auth::do_logout;
use crate::transport::leptos::signals::nav::use_blocking_navigate;
use crate::transport::leptos::signals::session::use_session;
use crate::transport::leptos::signals::toast::use_toast;

#[component]
pub fn LogoutPage() -> impl IntoView {
    let session_store = use_session();
    let toasts = use_toast();
    let navigate = StoredValue::new_local(use_blocking_navigate());
    let last_error = RwSignal::new(None);

    Effect::new(move |_| {
        spawn_local(async move {
            match do_logout().await {
                Ok(()) => {
                    session_store.set(None);
                    toasts.success("Signed out.");
                    navigate.with_value(|nav| nav(RouteName::Login.path().as_ref()));
                }
                Err(err) => {
                    err.log();
                    let msg: String = format!("{}", err);
                    toasts.error(msg);
                    last_error.set(Some(err));
                }
            }
        });
    });

    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Cards>
                <h1>"Signing out…"</h1>
                {move || last_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}
            </PageShell>
        </AuthGuard>
    }
}
