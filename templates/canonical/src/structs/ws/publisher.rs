use std::{marker::PhantomData, sync::Arc};

use serde::Serialize;

use crate::structs::ws::registry::Registry;

pub struct Channel<T: Serialize + Send + Sync + 'static> {
    pub topic: String,
    pub registry: Arc<Registry>,
    pub _phantom: PhantomData<fn() -> T>,
}
