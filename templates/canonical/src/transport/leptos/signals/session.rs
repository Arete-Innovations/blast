use leptos::prelude::*;

use crate::structs::leptos::SessionStore;

pub fn provide_session_store() -> SessionStore {
    let store = SessionStore::new();
    provide_context(store);
    store
}

pub fn use_session() -> SessionStore {
    match use_context::<SessionStore>() {
        Some(store) => store,
        None => {
            let store = SessionStore::new();
            provide_context(store);
            store
        }
    }
}
