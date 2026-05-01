use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;

use crate::meltdown::MeltDown;
use crate::structs::leptos::RegisterInput;
use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell};
use crate::transport::leptos::data::auth::do_register;
use crate::transport::leptos::signals::session::use_session;
use crate::transport::leptos::signals::toast::use_toast;

#[component]
pub fn RegisterPage() -> impl IntoView {
    let session_store = use_session();
    let toasts = use_toast();
    let navigate = StoredValue::new_local(use_navigate());

    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let pending = RwSignal::new(false);
    let last_error: RwSignal<Option<MeltDown>> = RwSignal::new(None);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if pending.get_untracked() {
            return;
        }
        if password.get_untracked() != confirm.get_untracked() {
            toasts.error("Passwords do not match.");
            return;
        }
        pending.set(true);
        last_error.set(None);
        let input = RegisterInput {
            email: email.get_untracked(),
            password: password.get_untracked(),
        };
        spawn_local(async move {
            let result = do_register(input).await;
            pending.set(false);
            match result {
                Ok(session) => {
                    session_store.set(Some(session));
                    toasts.success("Welcome.");
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
        <AuthGuard mode=AuthGuardMode::AnonOnly>
            <PageShell layout=PageLayout::Cards>
                <h1>"Register"</h1>
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
                    <label>
                        "Confirm password "
                        <input
                            type="password"
                            required=true
                            prop:value=move || confirm.get()
                            on:input=move |ev| confirm.set(event_target_value(&ev))
                        />
                    </label>
                    <button type="submit" disabled=move || pending.get()>
                        {move || if pending.get() { "Creating…" } else { "Create account" }}
                    </button>
                    {move || last_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}
                </form>
                <p><a href="/login">"Already have an account? Login"</a></p>
            </PageShell>
        </AuthGuard>
    }
}
