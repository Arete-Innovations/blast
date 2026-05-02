use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::meltdown::MeltDown;
use crate::structs::leptos::{ButtonKind, LoginInput, PageLayout, RouteName};
use crate::transport::leptos::components::{AuthCard, AuthCardAlt, AuthGuard, AuthGuardMode, Button, ErrorBanner, FormGroup, PageShell};
use crate::transport::leptos::data::auth::do_login;
use crate::transport::leptos::signals::nav::use_blocking_navigate;
use crate::transport::leptos::signals::session::use_session;
use crate::transport::leptos::signals::toast::use_toast;

#[component]
pub fn LoginPage() -> impl IntoView {
    let session_store = use_session();
    let toasts = use_toast();
    let navigate = StoredValue::new_local(use_blocking_navigate());

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
                Ok(session) => {
                    session_store.set(Some(session));
                    toasts.success("Signed in.");
                    navigate.with_value(|nav| nav(RouteName::Dashboard.path().as_ref()));
                }
                Err(err) => {
                    err.log();
                    let msg: String = err.user_message();
                    toasts.error(msg);
                    last_error.set(Some(err));
                }
            }
        });
    };

    view! {
        <AuthGuard mode=AuthGuardMode::AnonOnly>
            <PageShell layout=PageLayout::Bleed>
            <AuthCard title="Sign in".to_string() lede=Some("Welcome back. Use your work email.".to_string())>
                <form on:submit=on_submit>
                    <FormGroup label="Email".to_string() for_id="login_email".to_string()>
                        <input
                            id="login_email"
                            type="email"
                            autocomplete="email"
                            required=true
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                        />
                    </FormGroup>
                    <FormGroup label="Password".to_string() for_id="login_password".to_string()>
                        <input
                            id="login_password"
                            type="password"
                            autocomplete="current-password"
                            required=true
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                        />
                    </FormGroup>
                    {move || last_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}
                    <Button kind=ButtonKind::Primary kind_attr="submit".to_string() full=true disabled=pending.get()>
                        {move || match pending.get() {
                            true => "Signing in…",
                            false => "Sign in",
                        }}
                    </Button>
                </form>
                <AuthCardAlt>
                    "New here? "
                    <a href={RouteName::Register.path().to_string()}>"Create an account"</a>
                </AuthCardAlt>
            </AuthCard>
            </PageShell>
        </AuthGuard>
    }
}
