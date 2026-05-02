use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{BlastError, BlastResult};

/// Rust strict + reserved keywords. Hand-edited RON state files or wizard input
/// using any of these as a resource/field name causes codegen to emit
/// unparseable `pub mod <kw>` or `<kw>: T` declarations, surfacing only as
/// cryptic Rust syntax errors from the user's `cargo check`. Validation here
/// catches them at state-load + wizard-input time with a blast-level
/// diagnostic instead.
pub const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
    "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "try",
    "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "gen",
];

pub fn is_rust_keyword(s: &str) -> bool {
    RUST_KEYWORDS.contains(&s)
}

pub fn is_snake_case_ident(s: &str) -> bool {
    let trimmed = s.trim();
    let first = match trimmed.chars().next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    trimmed.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn validate_ident(kind: &str, s: &str) -> BlastResult<()> {
    if !is_snake_case_ident(s) {
        return Err(BlastError::Invalid(format!(
            "{kind} name '{s}' must be snake_case (^[a-z][a-z0-9_]*$)."
        )));
    }
    if is_rust_keyword(s) {
        return Err(BlastError::Invalid(format!(
            "{kind} name '{s}' is a Rust keyword; rename to avoid generated-code conflicts."
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FieldName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SqlType(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuthScopeField(String);

impl ResourceName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn try_new(value: impl Into<String>) -> BlastResult<Self> {
        let s: String = value.into();
        validate_ident("Resource", &s)?;
        Ok(Self(s))
    }
    pub fn validate(&self) -> BlastResult<()> {
        validate_ident("Resource", &self.0)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FieldName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn try_new(value: impl Into<String>) -> BlastResult<Self> {
        let s: String = value.into();
        validate_ident("Field", &s)?;
        Ok(Self(s))
    }
    pub fn validate(&self) -> BlastResult<()> {
        validate_ident("Field", &self.0)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SqlType {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AuthScopeField {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn validate(&self) -> BlastResult<()> {
        validate_ident("Auth scope field", &self.0)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for FieldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for SqlType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for AuthScopeField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ResourceName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for FieldName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SqlType {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for AuthScopeField {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for ResourceName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<String> for FieldName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<String> for SqlType {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<String> for AuthScopeField {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ResourceName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&str> for FieldName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&str> for SqlType {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&str> for AuthScopeField {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
