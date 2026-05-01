use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

pub use crate::structs::leptos::{NavState, NavStore};

pub fn provide_nav_store() -> NavStore {
    let store = NavStore::new();
    provide_context(store);
    store
}

pub fn use_nav_state() -> RwSignal<NavState> {
    use_nav_store().state
}

pub fn use_nav_store() -> NavStore {
    match use_context::<NavStore>() {
        Some(store) => store,
        None => {
            let store = NavStore::new();
            provide_context(store);
            store
        }
    }
}

pub fn use_blocking_navigate() -> impl Fn(&str) + Copy {
    let store = use_nav_store();
    let navigate = StoredValue::new_local(use_navigate());
    move |target: &str| {
        store.target.set(Some(target.to_string()));
        store.state.set(NavState::Pending(now_ms()));
        let target_owned = target.to_string();
        navigate.with_value(|nav| nav(&target_owned, Default::default()));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now_ms() -> f64 {
    use crate::cata_log;
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as f64,
        Err(err) => {
            cata_log!(Warning, format!("now_ms: system clock before unix epoch: {}", err));
            f64::NAN
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(target_arch = "wasm32")]
pub fn schedule_idle(store: NavStore, delay_ms: i32) {
    use crate::meltdown::{MeltDown, MeltType};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let callback = Closure::once_into_js(move || {
        if matches!(store.state.get_untracked(), NavState::Settled) {
            store.state.set(NavState::Idle);
            store.target.set(None);
        }
    });
    if let Err(err) = window.set_timeout_with_callback_and_timeout_and_arguments_0(callback.as_ref().unchecked_ref(), delay_ms) {
        MeltDown::new(MeltType::Unexpected("nav_schedule_idle".to_string()), format!("setTimeout failed: {:?}", err)).log();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn schedule_idle(_store: NavStore, _delay_ms: i32) {}
