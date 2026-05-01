use leptos::prelude::*;
use leptos_router::hooks::use_location;
use stylance::import_crate_style;

use crate::transport::leptos::signals::nav::{schedule_idle, use_nav_store, NavState};

import_crate_style!(style, "src/transport/leptos/components/nav_progress.module.scss");

#[component]
pub fn NavProgress() -> impl IntoView {
    let store = use_nav_store();
    let pathname = use_location().pathname;

    Effect::new(move |_| {
        let current = pathname.get();
        let target = store.target.get_untracked();
        let state = store.state.get_untracked();
        if let (NavState::Pending(_start), Some(target_path)) = (state, target.as_ref()) {
            if &current == target_path {
                store.state.set(NavState::Settled);
                schedule_idle(store, 200);
            }
        }
    });

    let class = move || {
        let state_class = match store.state.get() {
            NavState::Idle => style::idle,
            NavState::Pending(_) => style::pending,
            NavState::Settled => style::settled,
        };
        format!("{} {}", style::bar, state_class)
    };

    view! {
        <div class=class role="progressbar" aria-hidden="true"></div>
    }
}
