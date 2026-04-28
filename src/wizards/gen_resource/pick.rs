use dialoguer::{theme::ColorfulTheme, FuzzySelect};

use crate::{
    error::{BlastError, BlastResult},
    schema_parser::ParsedTable,
};

pub fn select_table<'a>(tables: &'a [ParsedTable], preselected: Option<&str>) -> BlastResult<&'a ParsedTable> {
    match preselected {
        Some(name) => find_table(tables, name),
        None => prompt_table(tables),
    }
}

fn find_table<'a>(tables: &'a [ParsedTable], name: &str) -> BlastResult<&'a ParsedTable> {
    let found = tables.iter().find(|t| t.name == name);
    match found {
        Some(t) => Ok(t),
        None => Err(BlastError::NotFound(format!(
            "table `{}` not present in schema.rs (have: {})",
            name,
            tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", "),
        ))),
    }
}

fn prompt_table(tables: &[ParsedTable]) -> BlastResult<&ParsedTable> {
    let theme = ColorfulTheme::default();
    let labels: Vec<String> = tables.iter().map(|t| format!("{} ({} cols)", t.name, t.columns.len())).collect();
    let idx = FuzzySelect::with_theme(&theme).with_prompt("Pick a table to author/edit a resource for").items(&labels).default(0).interact()?;
    let picked = tables.get(idx);
    match picked {
        Some(t) => Ok(t),
        None => Err(BlastError::Invalid(format!("FuzzySelect returned out-of-range index {idx}"))),
    }
}
