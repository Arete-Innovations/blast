use leptos::prelude::*;

use crate::structs::leptos::toast::{Toast, ToastKind};

#[derive(Clone, Copy)]
pub struct ToastStore {
    items: RwSignal<Vec<Toast>>,
    next_id: RwSignal<u64>,
}

impl ToastStore {
    pub fn new() -> Self {
        Self {
            items: RwSignal::new(Vec::new()),
            next_id: RwSignal::new(1),
        }
    }

    pub fn list(&self) -> ReadSignal<Vec<Toast>> {
        self.items.read_only()
    }

    pub fn push(&self, kind: ToastKind, message: impl Into<String>) {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        self.items.update(|v| {
            v.push(Toast {
                id,
                kind,
                message: message.into(),
            })
        });
    }

    pub fn success(&self, message: impl Into<String>) {
        self.push(ToastKind::Success, message);
    }

    pub fn error(&self, message: impl Into<String>) {
        self.push(ToastKind::Error, message);
    }

    pub fn info(&self, message: impl Into<String>) {
        self.push(ToastKind::Info, message);
    }

    pub fn dismiss(&self, id: u64) {
        self.items.update(|v| v.retain(|t| t.id != id));
    }
}

impl Default for ToastStore {
    fn default() -> Self {
        Self::new()
    }
}
