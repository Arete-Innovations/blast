use std::sync::Arc;

use canonical::structs::fuses::running_guard::RunningGuard;
use dashmap::DashMap;

#[test]
fn drop_clears_owned_entry() {
    let map: Arc<DashMap<String, ()>> = Arc::new(DashMap::new());
    map.insert("fuse.alpha".to_string(), ());
    assert!(map.contains_key("fuse.alpha"));

    {
        let _guard: RunningGuard = RunningGuard {
            map: map.clone(),
            name: "fuse.alpha".to_string(),
        };
        assert!(map.contains_key("fuse.alpha"), "guard does not remove on construction");
    }

    assert!(!map.contains_key("fuse.alpha"), "guard removes entry on drop");
}

#[test]
fn drop_only_removes_named_entry() {
    let map: Arc<DashMap<String, ()>> = Arc::new(DashMap::new());
    map.insert("fuse.alpha".to_string(), ());
    map.insert("fuse.beta".to_string(), ());

    {
        let _guard: RunningGuard = RunningGuard {
            map: map.clone(),
            name: "fuse.alpha".to_string(),
        };
    }

    assert!(!map.contains_key("fuse.alpha"));
    assert!(map.contains_key("fuse.beta"), "guard must not touch unrelated entries");
}

#[test]
fn drop_on_missing_entry_is_noop() {
    let map: Arc<DashMap<String, ()>> = Arc::new(DashMap::new());

    {
        let _guard: RunningGuard = RunningGuard {
            map: map.clone(),
            name: "fuse.never_inserted".to_string(),
        };
    }

    assert_eq!(map.len(), 0, "drop on absent key must not panic or insert");
}

#[tokio::test]
async fn panic_in_spawned_task_still_drops_guard() {
    let map: Arc<DashMap<String, ()>> = Arc::new(DashMap::new());
    map.insert("fuse.kaboom".to_string(), ());

    let map_for_task = map.clone();
    let handle = tokio::spawn(async move {
        let _guard: RunningGuard = RunningGuard {
            map: map_for_task,
            name: "fuse.kaboom".to_string(),
        };
        panic!("synthetic fuse panic");
    });

    let join_result = handle.await;
    assert!(join_result.is_err(), "spawned task must report panic via JoinError");

    assert!(!map.contains_key("fuse.kaboom"), "RAII guard must clear running set even when task panics");
}

#[tokio::test]
async fn ok_return_in_spawned_task_drops_guard() {
    let map: Arc<DashMap<String, ()>> = Arc::new(DashMap::new());
    map.insert("fuse.normal".to_string(), ());

    let map_for_task = map.clone();
    let handle = tokio::spawn(async move {
        let _guard: RunningGuard = RunningGuard {
            map: map_for_task,
            name: "fuse.normal".to_string(),
        };
    });

    handle.await.expect("normal completion must not error");
    assert!(!map.contains_key("fuse.normal"), "guard fires on Ok-return");
}
