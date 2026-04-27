use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

use crate::{
    cata_log,
    meltdown::{MeltDown, MeltType},
    structs::services::storage::*,
};

const DEFAULT_ROOT: &str = "./storage";

impl Storage {
    pub fn from_env() -> Result<Storage, MeltDown> {
        let root = match env::var("STORAGE_ROOT") {
            Ok(val) => val,
            Err(e) => {
                cata_log!(Debug, format!("STORAGE_ROOT unset, using default: {}", e));
                DEFAULT_ROOT.to_string()
            }
        };
        let root = PathBuf::from(root);

        fs::create_dir_all(&root).map_err(|e| MeltDown::new(MeltType::ConfigurationError, format!("storage root unusable: {}", root.display())).with_source(e))?;

        cata_log!(Info, format!("storage root: {}", root.display()));

        Ok(Storage { root })
    }

    pub fn put(&self, path: &str, bytes: &[u8]) -> Result<(), MeltDown> {
        let abs = self.resolve(path)?;

        abs.parent()
            .map(|parent| fs::create_dir_all(parent).map_err(|e| MeltDown::new(MeltType::FileOperationFailed, format!("create parent dirs for {}", path)).with_source(e)))
            .transpose()?;

        fs::write(&abs, bytes).map_err(|e| MeltDown::new(MeltType::FileOperationFailed, format!("write {}", path)).with_source(e))?;

        Ok(())
    }

    pub fn get(&self, path: &str) -> Result<Vec<u8>, MeltDown> {
        let abs = self.resolve(path)?;

        match fs::read(&abs) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(MeltDown::new(MeltType::FileNotFound, path.to_string()).with_source(e)),
            Err(e) => Err(MeltDown::new(MeltType::FileOperationFailed, format!("read {}", path)).with_source(e)),
        }
    }

    pub fn delete(&self, path: &str) -> Result<(), MeltDown> {
        let abs = self.resolve(path)?;

        match fs::remove_file(&abs) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(MeltDown::new(MeltType::FileOperationFailed, format!("delete {}", path)).with_source(e)),
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
        self.resolve(path).is_ok_and(|abs| abs.exists())
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, MeltDown> {
        if path.is_empty() {
            return Err(MeltDown::new(MeltType::FilePermissionDenied, "empty path"));
        }
        if path.starts_with('/') {
            return Err(MeltDown::new(MeltType::FilePermissionDenied, format!("absolute path rejected: {}", path)));
        }

        let candidate = Path::new(path);
        for component in candidate.components() {
            match component {
                Component::ParentDir => {
                    return Err(MeltDown::new(MeltType::FilePermissionDenied, format!("parent-dir traversal rejected: {}", path)));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(MeltDown::new(MeltType::FilePermissionDenied, format!("absolute path rejected: {}", path)));
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
            return Err(MeltDown::new(MeltType::FileOperationFailed, format!("read_dir {}", dir.display())).with_source(e));
        }
    };

    for entry in entries {
        let entry = entry.map_err(|e| MeltDown::new(MeltType::FileOperationFailed, format!("read_dir entry under {}", dir.display())).with_source(e))?;
        let path = entry.path();
        let ft = entry
            .file_type()
            .map_err(|e| MeltDown::new(MeltType::FileOperationFailed, format!("file_type {}", path.display())).with_source(e))?;
        if ft.is_dir() {
            walk(root, &path, out)?;
        } else if ft.is_file() {
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let s: String = rel
                .components()
                .filter_map(|c| match c {
                    Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
                    Component::ParentDir | Component::RootDir | Component::Prefix(_) | Component::CurDir => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            if !s.is_empty() {
                out.push(s);
            }
        }
    }

    Ok(())
}
