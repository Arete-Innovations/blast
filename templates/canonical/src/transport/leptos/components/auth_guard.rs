use leptos::prelude::*;
use leptos_router::components::Redirect;

use crate::structs::auth::role::Role;
use crate::structs::leptos::{AuthGuardMode, RouteName};
use crate::transport::leptos::signals::session::use_session;

#[component]
pub fn AuthGuard(mode: AuthGuardMode, children: ChildrenFn) -> impl IntoView {
    let session_store = use_session();
    let children_stored = StoredValue::new(children);

    move || {
        let snapshot = session_store.get();
        let authed = snapshot.is_some();
        let is_admin = matches!(snapshot.as_ref().map(|s| s.role), Some(Role::Admin));
        match mode {
            AuthGuardMode::Public => children_stored.with_value(|c| c()).into_any(),
            AuthGuardMode::AnonOnly => {
                if authed {
                    let path = RouteName::Dashboard.path().to_string();
                    view! { <Redirect path=path/> }.into_any()
                } else {
                    children_stored.with_value(|c| c()).into_any()
                }
            }
            AuthGuardMode::Required => {
                if authed {
                    children_stored.with_value(|c| c()).into_any()
                } else {
                    let path = RouteName::Login.path().to_string();
                    view! { <Redirect path=path/> }.into_any()
                }
            }
            AuthGuardMode::AdminOnly => {
                if !authed {
                    let path = RouteName::Login.path().to_string();
                    view! { <Redirect path=path/> }.into_any()
                } else if is_admin {
                    children_stored.with_value(|c| c()).into_any()
                } else {
                    let path = RouteName::Welcome.path().to_string();
                    view! { <Redirect path=path/> }.into_any()
                }
            }
        }
    }
}
