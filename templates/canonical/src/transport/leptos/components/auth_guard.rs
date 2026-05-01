use leptos::prelude::*;
use leptos_router::components::Redirect;

use crate::structs::auth::role::Role;
use crate::structs::auth::session_context::SessionContext;
use crate::structs::leptos::AuthGuardMode;

#[component]
pub fn AuthGuard(mode: AuthGuardMode, children: ChildrenFn) -> impl IntoView {
    let allowed = match mode {
        AuthGuardMode::Public => true,
        AuthGuardMode::Required => session_present(),
        AuthGuardMode::AdminOnly => session_is_admin(),
    };

    view! {
        <Show
            when=move || allowed
            fallback=|| view! { <Redirect path="/login"/> }
        >
            {children()}
        </Show>
    }
}

fn session_present() -> bool {
    let ctx: Option<Option<SessionContext>> = use_context();
    matches!(ctx, Some(Some(_)))
}

fn session_is_admin() -> bool {
    let ctx: Option<Option<SessionContext>> = use_context();
    match ctx {
        Some(Some(s)) => matches!(s.role, Role::Admin),
        _none_or_anon => false,
    }
}
