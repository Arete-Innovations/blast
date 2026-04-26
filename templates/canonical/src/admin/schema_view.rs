
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdminConfig {
    pub tables: Vec<AdminTable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminTable {
    pub name: String,
    pub display_name: String,
    pub columns: Vec<AdminColumn>,
    pub list_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminColumn {
    pub name: String,
    pub public: bool,
}

impl AdminConfig {
    pub fn from_tables(tables: Vec<AdminTable>) -> Self {
        Self { tables }
    }

    pub fn table(&self, name: &str) -> Option<&AdminTable> {
        self.tables.iter().find(|t| t.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_tables_round_trips() {
        let tables = vec![AdminTable {
            name: "users".into(),
            display_name: "Users".into(),
            columns: vec![AdminColumn { name: "id".into(), public: true }],
            list_columns: vec!["id".into()],
        }];
        let cfg = AdminConfig::from_tables(tables);
        assert_eq!(cfg.tables.len(), 1);
        assert!(cfg.table("users").is_some());
        assert!(cfg.table("missing").is_none());
    }
}
