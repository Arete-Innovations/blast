use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationSpec {
    Custom(CustomSpec),
    NewTable(NewTableSpec),
    AlterTable(AlterTableSpec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomSpec {
    pub name: String,
    pub up_sql: String,
    pub down_sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTableSpec {
    pub table: String,
    pub columns: Vec<ColumnSpec>,
    pub foreign_keys: Vec<ForeignKeySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlterTableSpec {
    pub table: String,
    pub additions: Vec<ColumnSpec>,
    pub foreign_keys: Vec<ForeignKeySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSpec {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
    pub unique: bool,
    pub default: Option<String>,
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKeySpec {
    pub column: String,
    pub referenced_table: String,
    pub referenced_column: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub migration_name: String,
    pub up_path: PathBuf,
    pub down_path: PathBuf,
}

impl MigrationSpec {
    pub fn migration_name(&self) -> String {
        match self {
            MigrationSpec::Custom(c) => c.name.clone(),
            MigrationSpec::NewTable(n) => format!("create_{}", n.table),
            MigrationSpec::AlterTable(a) => format!("alter_{}", a.table),
        }
    }
}
