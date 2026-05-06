
pub const LEPTOS_LOGIN: &str = r#"use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::meltdown::MeltDown;
use crate::structs::vendored::leptos::{ButtonKind, LoginInput, PageLayout, RouteName};
use crate::views::components::{AuthCard, AuthCardAlt, AuthGuard, AuthGuardMode, Button, ErrorBanner, FormGroup, PageShell};
use crate::transport::leptos::data::auth::do_login;
use crate::views::signals::nav::use_blocking_navigate;
use crate::views::signals::session::use_session;
use crate::views::signals::toast::use_toast;

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
                    session_store.set(session);
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
            <AuthCard title="Sign in".to_string() lede="Welcome back. Use your work email.".to_string()>
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
                    <Button kind=ButtonKind::Primary kind_attr="submit".to_string() full=true disabled=Signal::derive(move || pending.get())>
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
"#;

pub const LEPTOS_REGISTER: &str = r#"use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::meltdown::MeltDown;
use crate::structs::vendored::leptos::{ButtonKind, PageLayout, RegisterInput, RouteName};
use crate::views::components::{AuthCard, AuthCardAlt, AuthGuard, AuthGuardMode, Button, ErrorBanner, FormGroup, PageShell};
use crate::transport::leptos::data::auth::do_register;
use crate::views::signals::nav::use_blocking_navigate;
use crate::views::signals::session::use_session;
use crate::views::signals::toast::use_toast;

#[component]
pub fn RegisterPage() -> impl IntoView {
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
        let input = RegisterInput {
            email: email.get_untracked(),
            password: password.get_untracked(),
        };
        spawn_local(async move {
            let result = do_register(input).await;
            pending.set(false);
            match result {
                Ok(session) => {
                    session_store.set(session);
                    toasts.success("Welcome.");
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
            <AuthCard title="Create account".to_string() lede="One minute, then you're in.".to_string()>
                <form on:submit=on_submit>
                    <FormGroup label="Email".to_string() for_id="register_email".to_string()>
                        <input
                            id="register_email"
                            type="email"
                            autocomplete="email"
                            required=true
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                        />
                    </FormGroup>
                    <FormGroup label="Password".to_string() for_id="register_password".to_string()>
                        <input
                            id="register_password"
                            type="password"
                            autocomplete="new-password"
                            required=true
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                        />
                    </FormGroup>
                    {move || last_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}
                    <Button kind=ButtonKind::Primary kind_attr="submit".to_string() full=true disabled=Signal::derive(move || pending.get())>
                        {move || match pending.get() {
                            true => "Creating…",
                            false => "Create account",
                        }}
                    </Button>
                </form>
                <AuthCardAlt>
                    "Already have an account? "
                    <a href={RouteName::Login.path().to_string()}>"Sign in"</a>
                </AuthCardAlt>
            </AuthCard>
            </PageShell>
        </AuthGuard>
    }
}
"#;

pub const LEPTOS_LOGOUT: &str = r#"use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::structs::vendored::leptos::RouteName;
use crate::views::components::{AuthGuardMode, AuthGuard, ErrorBanner, PageLayout, PageShell};
use crate::transport::leptos::data::auth::do_logout;
use crate::views::signals::nav::use_blocking_navigate;
use crate::views::signals::session::use_session;
use crate::views::signals::toast::use_toast;

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
                    session_store.clear();
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
"#;

pub const LEPTOS_PROFILE: &str = r#"use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::vendored::leptos::{AvatarSize, BadgeColor, ButtonKind, PageLayout};
use crate::views::components::cells::BadgeCell;
use crate::views::components::{AppShell, AuthGuard, AuthGuardMode, AvatarCell, Button, Card, PageShell};
use crate::views::signals::session::use_session;

import_crate_style!(style, "src/transport/leptos/pages/generated/profile.module.scss");

#[component]
pub fn ProfilePage() -> impl IntoView {
    let session_signal = use_session().signal();
    let user_id = move || match session_signal.get() {
        Some(s) => s.user_id.to_string(),
        None => "—".to_string(),
    };
    let role = move || match session_signal.get() {
        Some(s) => format!("{:?}", s.role),
        None => "—".to_string(),
    };
    view! {
        <AuthGuard mode=AuthGuardMode::Required>
            <PageShell layout=PageLayout::Bleed>
            <AppShell title="Profile".to_string()>
                <Card>
                    <div class=style::identity>
                        <AvatarCell name="You".to_string() size=AvatarSize::Lg/>
                        <div class=style::identity_meta>
                            <h3 class=style::identity_name>"Your account"</h3>
                            <p class=style::identity_email>"Manage your identity and security settings."</p>
                        </div>
                    </div>
                </Card>

                <Card title="Identity".to_string()>
                    <div class=style::section>
                        <div class=style::row>
                            <span class=style::label>"User ID"</span>
                            <span class=style::value>{user_id}</span>
                        </div>
                        <div class=style::row>
                            <span class=style::label>"Role"</span>
                            <span class=style::value>
                                <BadgeCell text=Signal::derive(role) color=BadgeColor::Info/>
                            </span>
                        </div>
                        <div class=style::row>
                            <span class=style::label>"Status"</span>
                            <span class=style::value>
                                <BadgeCell text="Active".to_string() color=BadgeColor::Success/>
                            </span>
                        </div>
                    </div>
                </Card>

                <Card title="Security".to_string()>
                    <p class=style::identity_email>"Reset your password or revoke active sessions."</p>
                    <div class=style::actions>
                        <Button kind=ButtonKind::Secondary>"Change password"</Button>
                        <Button kind=ButtonKind::Danger>"Revoke all sessions"</Button>
                    </div>
                </Card>
            </AppShell>
            </PageShell>
        </AuthGuard>
    }
}
"#;

pub const LEPTOS_PROFILE_SCSS: &str = r#".row {
    display: grid;
    grid-template-columns: 8rem 1fr;
    gap: var(--app-space-md);
    padding: var(--app-space-sm) 0;
    border-bottom: 0.0625rem solid var(--app-color-border-subtle);
    align-items: center;

    &:last-child { border-bottom: 0; }
}

.label {
    color: var(--app-color-fg-muted);
    font-size: var(--app-fs-sm);
}

.value {
    color: var(--app-color-fg);
    font-size: var(--app-fs-md);
}

.section {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-sm);
}

.identity {
    display: flex;
    align-items: center;
    gap: var(--app-space-md);
}

.identity_meta {
    display: flex;
    flex-direction: column;
    gap: var(--app-space-2xs, 0.125rem);
}

.identity_name {
    margin: 0;
    font-size: var(--app-fs-lg);
    font-weight: 600;
    color: var(--app-color-fg);
}

.identity_email {
    margin: 0;
    color: var(--app-color-fg-muted);
    font-size: var(--app-fs-sm);
}

.actions {
    display: flex;
    gap: var(--app-space-sm);
    flex-wrap: wrap;
}
"#;
