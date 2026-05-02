use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use ron::ser::PrettyConfig;

use crate::{
    error::{BlastError, BlastResult},
    state::{app::AppState, names::ResourceName, resource::ResourceState, upgraders},
};

pub const APP_FILE: &str = "app.ron";
pub const RESOURCES_DIR: &str = "resources";
pub const RESOURCE_EXT: &str = "ron";

pub fn app_path(state_dir: &Path) -> PathBuf {
    state_dir.join(APP_FILE)
}

pub fn resources_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(RESOURCES_DIR)
}

pub fn resource_path(state_dir: &Path, name: &ResourceName) -> PathBuf {
    resources_dir(state_dir).join(format!("{}.{}", name.as_str(), RESOURCE_EXT))
}

pub fn load_app(state_dir: &Path) -> BlastResult<AppState> {
    let path = app_path(state_dir);
    let raw = fs::read_to_string(&path)?;
    let mut value: AppState = ron::from_str(&raw)?;
    upgraders::upgrade_app(&mut value)?;
    value.canonicalize();
    Ok(value)
}

pub fn load_resource(state_dir: &Path, name: &ResourceName) -> BlastResult<ResourceState> {
    let path = resource_path(state_dir, name);
    let raw = fs::read_to_string(&path)?;
    let upgraded = upgraders::upgrade_resource_raw(&raw)?;
    let mut value: ResourceState = ron::from_str(&upgraded)?;
    upgraders::upgrade_resource(&mut value)?;
    value.canonicalize();
    Ok(value)
}

pub fn list_resources(state_dir: &Path) -> BlastResult<Vec<ResourceName>> {
    let dir = resources_dir(state_dir);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut names: Vec<ResourceName> = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = match path.extension() {
            Some(ext) => ext,
            None => continue,
        };
        if extension != OsStr::new(RESOURCE_EXT) {
            continue;
        }
        let stem = match path.file_stem() {
            Some(s) => s,
            None => continue,
        };
        let stem_str = match stem.to_str() {
            Some(s) => s,
            None => return Err(BlastError::Invalid(format!("non-utf8 resource filename: {}", path.display()))),
        };
        names.push(ResourceName::new(stem_str));
    }
    names.sort();
    Ok(names)
}

pub fn save_app(state_dir: &Path, app: &AppState) -> BlastResult<()> {
    fs::create_dir_all(state_dir)?;
    let mut canonical = app.clone();
    canonical.canonicalize();
    let body = serialize_pretty(&canonical)?;
    write_atomic(&app_path(state_dir), body.as_bytes())
}

pub fn save_resource(state_dir: &Path, res: &ResourceState) -> BlastResult<()> {
    let dir = resources_dir(state_dir);
    fs::create_dir_all(&dir)?;
    let mut canonical = res.clone();
    canonical.canonicalize();
    let body = serialize_pretty(&canonical)?;
    write_atomic(&resource_path(state_dir, &canonical.name), body.as_bytes())
}

fn serialize_pretty<T: serde::Serialize>(value: &T) -> BlastResult<String> {
    let config = PrettyConfig::new().depth_limit(64).indentor("  ".to_string()).struct_names(true);
    let body = ron::ser::to_string_pretty(value, config)?;
    Ok(format!("{body}\n"))
}

struct TempFileGuard {
    path: PathBuf,
    committed: bool,
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(_io) => {} // allow: best-effort cleanup of temp on write-error path; can't propagate from Drop
        }
    }
}

fn write_atomic(target: &Path, bytes: &[u8]) -> BlastResult<()> {
    let parent = match target.parent() {
        Some(p) => p,
        None => return Err(BlastError::Invalid(format!("target path has no parent: {}", target.display()))),
    };
    fs::create_dir_all(parent)?;

    let file_name = match target.file_name().and_then(OsStr::to_str) {
        Some(name) => name,
        None => return Err(BlastError::Invalid(format!("target path has no filename: {}", target.display()))),
    };
    let temp = parent.join(format!(".{file_name}.tmp"));
    let mut guard = TempFileGuard { path: temp.clone(), committed: false };

    let mut f = fs::File::create(&temp)?;
    f.write_all(bytes)?;
    f.sync_data()?;
    drop(f);

    fs::rename(&temp, target)?;
    guard.committed = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_file_guard_drop_without_commit_removes_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("orphan.tmp");
        fs::write(&path, b"partial bytes").expect("seed");
        assert!(path.exists());

        {
            let _g = TempFileGuard { path: path.clone(), committed: false };
        } // guard dropped here

        assert!(!path.exists(), "uncommitted guard must remove the temp file on drop");
    }

    #[test]
    fn temp_file_guard_drop_with_commit_preserves_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("kept.tmp");
        fs::write(&path, b"survived").expect("seed");

        {
            let mut g = TempFileGuard { path: path.clone(), committed: false };
            g.committed = true;
        } // guard dropped here, but committed

        assert!(path.exists(), "committed guard must NOT remove the file");
        let body = fs::read(&path).expect("read");
        assert_eq!(body, b"survived");
    }

    #[test]
    fn temp_file_guard_drop_on_missing_path_is_noop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("never_existed.tmp");
        assert!(!path.exists());

        {
            let _g = TempFileGuard { path: path.clone(), committed: false };
        }

        assert!(!path.exists(), "drop on missing path must not panic or create");
    }

    #[test]
    fn write_atomic_round_trip_writes_then_reads_same_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("nested").join("file.ron");
        let payload = b"ResourceState(schema_version: 3)\n";

        write_atomic(&target, payload).expect("write");
        assert!(target.exists());

        let read = fs::read(&target).expect("read");
        assert_eq!(read, payload);

        let parent = target.parent().expect("parent");
        let entries: Vec<_> = fs::read_dir(parent).expect("read_dir").flatten().map(|e| e.file_name().into_string().unwrap_or_default()).collect();
        let leftovers: Vec<&String> = entries.iter().filter(|n| n.starts_with('.')).collect();
        assert!(leftovers.is_empty(), "no .tmp left after success: {entries:?}");
    }
}
