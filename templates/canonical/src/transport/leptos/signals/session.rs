use leptos::prelude::*;

use crate::structs::auth::SessionContext;
use crate::structs::leptos::SessionStore;

pub fn provide_session_store() -> SessionStore {
    let initial: Option<SessionContext> = ssr_initial_session();
    let store = SessionStore::with_initial(initial);
    provide_context(store);
    store
}

#[cfg(not(target_arch = "wasm32"))]
fn ssr_initial_session() -> Option<SessionContext> {
    use_context::<crate::Ctx>().and_then(|ctx| ctx.session().cloned())
}

#[cfg(target_arch = "wasm32")]
fn ssr_initial_session() -> Option<SessionContext> {
    None
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
