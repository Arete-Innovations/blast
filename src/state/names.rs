use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FieldName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerbName(String);

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
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FieldName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl VerbName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl SqlType {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AuthScopeField {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_inner(self) -> String {
        self.0
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

impl fmt::Display for VerbName {
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

impl AsRef<str> for VerbName {
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

impl From<String> for VerbName {
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

impl From<&str> for VerbName {
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
