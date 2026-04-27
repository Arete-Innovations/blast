use std::fs;

use canonical::{meltdown::MeltType, structs::services::storage::Storage};

fn tmp_storage() -> Storage {
    let mut root = std::env::temp_dir();
    root.push(format!("catalyst-storage-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    Storage { root }
}

#[test]
fn rejects_absolute() {
    let s = tmp_storage();
    assert!(s.put("/etc/passwd", b"x").is_err());
}

#[test]
fn rejects_parent_traversal() {
    let s = tmp_storage();
    assert!(s.put("../escape.txt", b"x").is_err());
    assert!(s.put("a/../../escape.txt", b"x").is_err());
}

#[test]
fn rejects_empty() {
    let s = tmp_storage();
    assert!(s.put("", b"x").is_err());
}

#[test]
fn put_get_delete() {
    let s = tmp_storage();
    s.put("a/b/c.txt", b"hello").unwrap();
    assert!(s.exists("a/b/c.txt"));
    assert_eq!(s.get("a/b/c.txt").unwrap(), b"hello");
    s.delete("a/b/c.txt").unwrap();
    assert!(!s.exists("a/b/c.txt"));
    s.delete("a/b/c.txt").unwrap();
}

#[test]
fn list_with_prefix() {
    let s = tmp_storage();
    s.put("avatars/1.png", b"x").unwrap();
    s.put("avatars/2.png", b"y").unwrap();
    s.put("logos/a.svg", b"z").unwrap();

    let mut a = s.list("avatars/").unwrap();
    a.sort();
    assert_eq!(a, vec!["avatars/1.png", "avatars/2.png"]);

    let all = s.list("").unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn missing_get_is_filenotfound() {
    let s = tmp_storage();
    let err = s.get("nope.txt").unwrap_err();
    assert!(matches!(err.melt_type, MeltType::FileNotFound));
}
