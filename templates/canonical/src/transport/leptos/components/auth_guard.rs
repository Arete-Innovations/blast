use leptos::prelude::*;
use leptos_router::components::Redirect;

use crate::structs::auth::role::Role;
use crate::structs::leptos::AuthGuardMode;
use crate::transport::leptos::signals::session::use_session;

#[component]
pub fn AuthGuard(mode: AuthGuardMode, children: ChildrenFn) -> impl IntoView {
    let session_store = use_session();

    view! {
        <Show
            when=move || allowed(mode, session_store.get().as_ref())
            fallback=|| view! { <Redirect path="/login"/> }
        >
            {children()}
        </Show>
    }
}

fn allowed(mode: AuthGuardMode, session: Option<&crate::structs::auth::SessionContext>) -> bool {
    match mode {
        AuthGuardMode::Public => true,
        AuthGuardMode::Required => session.is_some(),
        AuthGuardMode::AdminOnly => session.is_some_and(|s| matches!(s.role, Role::Admin)),
    }
}
