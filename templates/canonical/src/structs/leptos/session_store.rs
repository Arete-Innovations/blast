use leptos::prelude::*;

use crate::structs::auth::SessionContext;

#[derive(Clone, Copy)]
pub struct SessionStore {
    inner: RwSignal<Option<SessionContext>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self { inner: RwSignal::new(None) }
    }

    pub fn get(&self) -> Option<SessionContext> {
        self.inner.get()
    }

    pub fn set(&self, value: Option<SessionContext>) {
        self.inner.set(value);
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
