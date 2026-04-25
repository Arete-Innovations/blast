use crate::error::{BlastError, BlastResult};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Variant {
    DB,
    Insertable,
    Patch,
    Public,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Validator {
    MinLen(usize),
    MaxLen(usize),
    MinValue(f64),
    MaxValue(f64),
    Regex(String),
    Email,
    Url,
    OneOf(Vec<String>),
    Required,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Validation {
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub max_len: Option<usize>,
    #[serde(default)]
    pub min_len: Option<usize>,
    #[serde(default)]
    pub min: Option<i64>,
    #[serde(default)]
    pub max: Option<i64>,
    #[serde(default)]
    pub enum_values: Option<Vec<String>>,
    #[serde(default)]
    pub validators: Vec<Validator>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldSpec {
    pub name: String,
    pub variants: Vec<Variant>,
    #[serde(default)]
    pub validation: Validation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum VerbKind {
    List,
    Get,
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Auth {
    Public,
    AuthRequired,
    AdminOnly,
    ScopedTo(String),
    Roles(Vec<String>),
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FilterSpec {
    #[serde(default)]
    pub paginated: bool,
    #[serde(default)]
    pub filterable_columns: Vec<String>,
    #[serde(default)]
    pub sortable_columns: Vec<String>,
    #[serde(default)]
    pub default_sort: Option<String>,
    #[serde(default)]
    pub max_page_size: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VerbSpec {
    pub kind: VerbKind,
    pub auth: Auth,
    #[serde(default)]
    pub filter: FilterSpec,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceIr {
    pub table: String,
    #[serde(default)]
    pub fields: Vec<FieldSpec>,
    #[serde(default)]
    pub verbs: Vec<VerbSpec>,
}

impl ResourceIr {
    pub fn list_verb(&self) -> Option<&VerbSpec> {
        self.verbs.iter().find(|v| v.kind == VerbKind::List)
    }
}

pub fn load_primer_ir(project_root: &Path) -> BlastResult<Vec<ResourceIr>> {
    let dir = project_root.join("target").join("primer");
    if !dir.is_dir() {
        return Err(BlastError::NotFound(format!(
            "primer IR directory missing: {}",
            dir.display()
        )));
    }

    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if matches!(path.extension(), Some(e) if e == "json") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut out: Vec<ResourceIr> = Vec::with_capacity(paths.len());
    for path in &paths {
        let raw = fs::read_to_string(path)?;
        let ir: ResourceIr = serde_json::from_str(&raw).map_err(|e| {
            BlastError::Invalid(format!(
                "failed to parse primer IR at {}: {}",
                path.display(),
                e
            ))
        })?;
        out.push(ir);
    }
    out.sort_by(|a, b| a.table.cmp(&b.table));
    Ok(out)
}
