use std::{
    ffi::OsStr,
    fs,
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
    fs::write(&temp, bytes)?;
    fs::rename(&temp, target)?;
    Ok(())
}
