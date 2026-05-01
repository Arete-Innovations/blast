#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    pub key: String,
    pub label: String,
}

impl TabItem {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }
}
