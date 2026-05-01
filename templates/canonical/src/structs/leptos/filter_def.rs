#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterKind {
    Text,
    Select(Vec<(String, String)>),
    Bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterDef {
    pub column: String,
    pub label: String,
    pub kind: FilterKind,
    pub placeholder: Option<String>,
}

impl FilterDef {
    pub fn text(column: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            label: label.into(),
            kind: FilterKind::Text,
            placeholder: None,
        }
    }

    pub fn select(column: impl Into<String>, label: impl Into<String>, options: Vec<(String, String)>) -> Self {
        Self {
            column: column.into(),
            label: label.into(),
            kind: FilterKind::Select(options),
            placeholder: None,
        }
    }

    pub fn bool(column: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            column: column.into(),
            label: label.into(),
            kind: FilterKind::Bool,
            placeholder: None,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
}
