
use std::{
    env,
    fs,
    path::{Component, Path, PathBuf},
};

use crate::{
    cata_log,
    meltdown::{MeltDown, MeltType},
};

const DEFAULT_ROOT: &str = "./storage";

pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn from_env() -> Result<Storage, MeltDown> {
        let root = env::var("STORAGE_ROOT").unwrap_or_else(|_| DEFAULT_ROOT.to_string());
        let root = PathBuf::from(root);

        fs::create_dir_all(&root).map_err(|e| {
            MeltDown::new(MeltType::ConfigurationError, format!("storage root unusable: {}", root.display()))
                .with_source(e)
        })?;

        cata_log!(Info, format!("storage root: {}", root.display()));

        Ok(Storage { root })
    }

    pub fn put(&self, path: &str, bytes: &[u8]) -> Result<(), MeltDown> {
        let abs = self.resolve(path)?;

        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                MeltDown::new(MeltType::FileOperationFailed, format!("create parent dirs for {}", path))
                    .with_source(e)
            })?;
        }

        fs::write(&abs, bytes).map_err(|e| {
            MeltDown::new(MeltType::FileOperationFailed, format!("write {}", path)).with_source(e)
        })?;

        Ok(())
    }

    pub fn get(&self, path: &str) -> Result<Vec<u8>, MeltDown> {
        let abs = self.resolve(path)?;

        match fs::read(&abs) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
                MeltDown::new(MeltType::FileNotFound, path.to_string()).with_source(e),
            ),
            Err(e) => Err(
                MeltDown::new(MeltType::FileOperationFailed, format!("read {}", path)).with_source(e),
            ),
        }
    }

    pub fn delete(&self, path: &str) -> Result<(), MeltDown> {
        let abs = self.resolve(path)?;

        match fs::remove_file(&abs) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(MeltDown::new(MeltType::FileOperationFailed, format!("delete {}", path))
                .with_source(e)),
        }
    }

    pub fn list(&self, prefix: &str) -> Result<Vec<String>, MeltDown> {
        let mut out = Vec::new();
        walk(&self.root, &self.root, &mut out)?;
        out.retain(|p| p.starts_with(prefix));
        out.sort();
        Ok(out)
    }

    pub fn exists(&self, path: &str) -> bool {
        match self.resolve(path) {
            Ok(abs) => abs.exists(),
            Err(_) => false,
        }
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, MeltDown> {
        if path.is_empty() {
            return Err(MeltDown::new(MeltType::FilePermissionDenied, "empty path"));
        }
        if path.starts_with('/') {
            return Err(MeltDown::new(
                MeltType::FilePermissionDenied,
                format!("absolute path rejected: {}", path),
            ));
        }

        let candidate = Path::new(path);
        for component in candidate.components() {
            match component {
                Component::ParentDir => {
                    return Err(MeltDown::new(
                        MeltType::FilePermissionDenied,
                        format!("parent-dir traversal rejected: {}", path),
                    ));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(MeltDown::new(
                        MeltType::FilePermissionDenied,
                        format!("absolute path rejected: {}", path),
                    ));
                }
                Component::Normal(_) | Component::CurDir => {}
            }
        }

        Ok(self.root.join(candidate))
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), MeltDown> {
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(MeltDown::new(
                MeltType::FileOperationFailed,
                format!("read_dir {}", dir.display()),
            )
            .with_source(e));
        }
    };

    for entry in entries {
        let entry = entry.map_err(|e| {
            MeltDown::new(MeltType::FileOperationFailed, format!("read_dir entry under {}", dir.display()))
                .with_source(e)
        })?;
        let path = entry.path();
        let ft = entry.file_type().map_err(|e| {
            MeltDown::new(MeltType::FileOperationFailed, format!("file_type {}", path.display()))
                .with_source(e)
        })?;
        if ft.is_dir() {
            walk(root, &path, out)?;
        } else if ft.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                let s: String = rel
                    .components()
                    .filter_map(|c| match c {
                        Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("/");
                if !s.is_empty() {
                    out.push(s);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
