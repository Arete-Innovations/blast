use leptos::prelude::*;

use crate::meltdown::MeltDown;

pub type ReactiveSignal<T> = RwSignal<Option<Result<T, MeltDown>>>;

pub struct PolledResource<T: 'static + Send + Sync> {
    pub signal: ReactiveSignal<T>,
    pub refetch_trigger: RwSignal<u32>,
}

impl<T: 'static + Send + Sync> Copy for PolledResource<T> {}

impl<T: 'static + Send + Sync> Clone for PolledResource<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static + Send + Sync> PolledResource<T> {
    pub fn refetch(self) {
        self.refetch_trigger.update(|n| *n = n.wrapping_add(1));
    }
}

pub struct LiveResource<T: 'static + Send + Sync> {
    pub signal: ReactiveSignal<T>,
}

impl<T: 'static + Send + Sync> Copy for LiveResource<T> {}

impl<T: 'static + Send + Sync> Clone for LiveResource<T> {
    fn clone(&self) -> Self {
        *self
    }
}
