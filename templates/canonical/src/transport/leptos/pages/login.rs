use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::structs::leptos::LoginInput;
use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell};
use crate::transport::leptos::data::auth::do_login;
use crate::transport::leptos::signals::session::use_session;
use crate::transport::leptos::signals::toast::use_toast;

#[component]
pub fn LoginPage() -> impl IntoView {
    let session_store = use_session();
    let toasts = use_toast();
    let navigate = use_navigate();

    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());

    let action = Action::new_local(move |input: &LoginInput| {
        let input = input.clone();
        async move { do_login(input).await }
    });

    Effect::new(move |_| match action.value().get() {
        Some(Ok(out)) => {
            session_store.set(Some(out.session.clone()));
            toasts.success("Signed in.");
            navigate("/dashboard", Default::default());
        }
        Some(Err(e)) => {
            let msg: String = format!("{}", e);
            toasts.error(msg);
        }
        None => {}
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        action.dispatch(LoginInput {
            email: email.get(),
            password: password.get(),
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
                    <button type="submit" disabled=move || action.pending().get()>
                        {move || if action.pending().get() { "Signing in…" } else { "Sign in" }}
                    </button>
                    {move || match action.value().get() {
                        Some(Err(e)) => Some(view! { <ErrorBanner error=e/> }.into_any()),
                        _ignored => None,
                    }}
                </form>
                <p><a href="/register">"Need an account? Register"</a></p>
            </PageShell>
        </AuthGuard>
    }
}
