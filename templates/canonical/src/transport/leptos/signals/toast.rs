use leptos::prelude::*;

pub use crate::structs::leptos::{Toast, ToastKind, ToastStore};

pub fn provide_toast_store() -> ToastStore {
    let store = ToastStore::new();
    provide_context(store);
    store
}

pub fn use_toast() -> ToastStore {
    match use_context::<ToastStore>() {
        Some(store) => store,
        None => {
            let store = ToastStore::new();
            provide_context(store);
            store
        }
    }
}
