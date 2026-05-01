use leptos::prelude::*;

use crate::structs::auth::{Role, SessionContext};

#[derive(Clone, Copy)]
pub struct SessionStore {
    inner: RwSignal<Option<SessionContext>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::with_initial(None)
    }

    pub fn with_initial(initial: Option<SessionContext>) -> Self {
        Self { inner: RwSignal::new(initial) }
    }

    pub fn get(&self) -> Option<SessionContext> {
        self.inner.get()
    }

    pub fn set(&self, value: Option<SessionContext>) {
        self.inner.set(value);
    }

    pub fn is_authed(&self) -> bool {
        self.inner.get().is_some()
    }

    pub fn has_role(&self, role: Role) -> bool {
        self.inner.get().is_some_and(|ctx| ctx.role == role)
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
