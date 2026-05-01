use leptos::prelude::*;

pub use crate::structs::leptos::{Toast, ToastKind, ToastStore};

pub fn provide_toast_store() -> ToastStore {
    let store = ToastStore::new();
    provide_context(store);
    store
}

pub fn use_toast() -> ToastStore {
    match use_context::<ToastStore>() {
        Some(store) => store,
        None => {
            let store = ToastStore::new();
            provide_context(store);
            store
        }
    }
}

pub fn success(msg: impl Into<String>) {
    use_toast().success(msg);
}

pub fn error(msg: impl Into<String>) {
    use_toast().error(msg);
}

pub fn info(msg: impl Into<String>) {
    use_toast().info(msg);
}

pub fn warning(msg: impl Into<String>) {
    use_toast().warning(msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use leptos::reactive::owner::Owner;

    #[test]
    fn helpers_route_to_each_kind() {
        let owner = Owner::new();
        owner.with(|| {
            let store = ToastStore::new();
            store.success("ok");
            store.error("nope");
            store.info("fyi");
            store.warning("careful");
            let items = store.list().get_untracked();
            assert_eq!(items.len(), 4);
            assert_eq!(items[0].kind, ToastKind::Success);
            assert_eq!(items[1].kind, ToastKind::Error);
            assert_eq!(items[2].kind, ToastKind::Info);
            assert_eq!(items[3].kind, ToastKind::Warning);
            assert_eq!(items[0].message, "ok");
            assert_eq!(items[3].message, "careful");
        });
    }
}
