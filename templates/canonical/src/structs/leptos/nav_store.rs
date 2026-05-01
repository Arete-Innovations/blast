use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavState {
    Idle,
    Pending(f64),
    Settled,
}

#[derive(Clone, Copy)]
pub struct NavStore {
    pub state: RwSignal<NavState>,
    pub target: RwSignal<Option<String>>,
}

impl NavStore {
    pub fn new() -> Self {
        Self {
            state: RwSignal::new(NavState::Idle),
            target: RwSignal::new(None),
        }
    }
}

impl Default for NavStore {
    fn default() -> Self {
        Self::new()
    }
}
