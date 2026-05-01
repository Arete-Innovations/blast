#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
pub struct WasmDrop<T: 'static>(RefCell<Option<T>>);

#[cfg(target_arch = "wasm32")]
impl<T: 'static> WasmDrop<T> {
    pub fn new(value: T) -> Self {
        Self(RefCell::new(Some(value)))
    }

    pub fn consume(&self) {
        self.0.borrow_mut().take();
    }
}

#[cfg(target_arch = "wasm32")]
unsafe impl<T: 'static> Send for WasmDrop<T> {}

#[cfg(target_arch = "wasm32")]
unsafe impl<T: 'static> Sync for WasmDrop<T> {}

#[cfg(target_arch = "wasm32")]
pub struct WasmCleanup(RefCell<Option<Box<dyn FnOnce() + 'static>>>);

#[cfg(target_arch = "wasm32")]
impl WasmCleanup {
    pub fn new(f: Box<dyn FnOnce() + 'static>) -> Self {
        Self(RefCell::new(Some(f)))
    }

    pub fn consume(&self) {
        let taken = self.0.borrow_mut().take();
        match taken {
            Some(f) => f(),
            None => (),
        }
    }
}

#[cfg(target_arch = "wasm32")]
unsafe impl Send for WasmCleanup {}

#[cfg(target_arch = "wasm32")]
unsafe impl Sync for WasmCleanup {}
