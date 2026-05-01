use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::meltdown::MeltDown;
use crate::structs::leptos::LoginInput;
use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell};
use crate::transport::leptos::data::auth::do_login;
use crate::transport::leptos::signals::session::use_session;
use crate::transport::leptos::signals::toast::use_toast;

#[component]
pub fn LoginPage() -> impl IntoView {
    let session_store = use_session();
    let toasts = use_toast();
    let navigate = StoredValue::new_local(use_navigate());

    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let pending = RwSignal::new(false);
    let last_error: RwSignal<Option<MeltDown>> = RwSignal::new(None);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if pending.get_untracked() {
            return;
        }
        pending.set(true);
        last_error.set(None);
        let input = LoginInput {
            email: email.get_untracked(),
            password: password.get_untracked(),
        };
        spawn_local(async move {
            let result = do_login(input).await;
            pending.set(false);
            match result {
                Ok(out) => {
                    session_store.set(Some(out.session.clone()));
                    toasts.success("Signed in.");
                    navigate.with_value(|nav| nav("/dashboard", Default::default()));
                }
                Err(err) => {
                    err.log();
                    let msg: String = format!("{}", err);
                    toasts.error(msg);
                    last_error.set(Some(err));
                }
            }
        });
    };

    view! {
        <AuthGuard mode=AuthGuardMode::Public>
            <PageShell layout=PageLayout::Cards>
                <h1>"Login"</h1>
                <form on:submit=on_submit>
                    <label>
                        "Email "
                        <input
                            type="email"
                            required=true
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                        />
                    </label>
                    <label>
                        "Password "
                        <input
                            type="password"
                            required=true
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                        />
                    </label>
                    <button type="submit" disabled=move || pending.get()>
                        {move || if pending.get() { "Signing in…" } else { "Sign in" }}
                    </button>
                    {move || last_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}
                </form>
                <p><a href="/register">"Need an account? Register"</a></p>
            </PageShell>
        </AuthGuard>
    }
}
