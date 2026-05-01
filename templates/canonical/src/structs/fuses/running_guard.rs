use std::sync::Arc;

use dashmap::DashMap;

pub struct RunningGuard {
    pub map: Arc<DashMap<String, ()>>,
    pub name: String,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.map.remove(&self.name);
    }
}
