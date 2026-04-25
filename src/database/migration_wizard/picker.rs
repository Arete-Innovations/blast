use crate::database::migration_wizard::spec::{
    AlterTableSpec, ColumnSpec, CustomSpec, ForeignKeySpec, MigrationSpec, NewTableSpec,
};
use crate::database::migration_wizard::sql;
use crate::error::{BlastError, BlastResult};
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Select};

const COLUMN_TYPES: &[&str] = &[
    "SERIAL", "INTEGER", "BIGINT", "SMALLINT", "VARCHAR", "TEXT", "CHAR", "BOOLEAN",
    "FLOAT", "DOUBLE PRECISION", "DECIMAL", "NUMERIC", "DATE", "TIME", "TIMESTAMP",
    "TIMESTAMPTZ", "UUID", "JSON", "JSONB", "ARRAY",
];

const ARRAY_ELEMENT_TYPES: &[&str] =
    &["INTEGER", "TEXT", "VARCHAR", "BOOLEAN", "FLOAT", "UUID"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationKind {
    NewTable,
    AlterTable,
    Custom,
    Cancel,
}

pub fn pick_spec() -> BlastResult<Option<MigrationSpec>> {
    let theme = ColorfulTheme::default();

    let kind = pick_kind(&theme)?;
    match kind {
        MigrationKind::Cancel => Ok(None),
        MigrationKind::Custom => {
            let name = prompt_text(&theme, "Enter a name for your custom migration")?;
            Ok(Some(MigrationSpec::Custom(CustomSpec {
                name,
                up_sql: sql::custom_up_template(),
                down_sql: sql::custom_down_template(),
            })))
        }
        MigrationKind::NewTable => pick_new_table(&theme).map(Some),
        MigrationKind::AlterTable => pick_alter_table(&theme),
    }
}

fn pick_kind(theme: &ColorfulTheme) -> BlastResult<MigrationKind> {
    let actions = ["Create New Table", "Alter Existing Table", "Custom Migration", "Cancel"];
    let index = Select::with_theme(theme)
        .with_prompt("What type of migration do you want to create?")
        .default(0)
        .items(&actions)
        .interact()?;
    match index {
        0 => Ok(MigrationKind::NewTable),
        1 => Ok(MigrationKind::AlterTable),
        2 => Ok(MigrationKind::Custom),
        3 => Ok(MigrationKind::Cancel),
        other => Err(BlastError::Invalid(format!(
            "unexpected migration kind index {}",
            other
        ))),
    }
}

fn pick_new_table(theme: &ColorfulTheme) -> BlastResult<MigrationSpec> {
    let table = prompt_text(theme, "Enter the new table name")?;

    let mut columns = vec![ColumnSpec {
        name: "id".to_string(),
        sql_type: "SERIAL".to_string(),
        nullable: false,
        unique: false,
        default: None,
        primary_key: true,
    }];

    let mut foreign_keys: Vec<ForeignKeySpec> = Vec::new();
    pick_columns_loop(theme, &mut columns, &mut foreign_keys, true)?;

    Ok(MigrationSpec::NewTable(NewTableSpec {
        table,
        columns,
        foreign_keys,
    }))
}

fn pick_alter_table(theme: &ColorfulTheme) -> BlastResult<Option<MigrationSpec>> {
    let existing_tables = crate::database::seeds::get_existing_tables();
    if existing_tables.is_empty() {
        return Err(BlastError::Invalid(
            "no existing tables found; create a new table first".to_string(),
        ));
    }

    let mut choices: Vec<String> = existing_tables.clone();
    choices.push("Cancel".to_string());

    let index = FuzzySelect::with_theme(theme)
        .with_prompt("Select a table to alter")
        .items(&choices)
        .default(0)
        .interact()?;

    if index == choices.len() - 1 {
        return Ok(None);
    }

    let table = existing_tables[index].clone();

    let mut additions: Vec<ColumnSpec> = Vec::new();
    let mut foreign_keys: Vec<ForeignKeySpec> = Vec::new();
    pick_columns_loop(theme, &mut additions, &mut foreign_keys, false)?;

    Ok(Some(MigrationSpec::AlterTable(AlterTableSpec {
        table,
        additions,
        foreign_keys,
    })))
}

fn pick_columns_loop(
    theme: &ColorfulTheme,
    columns: &mut Vec<ColumnSpec>,
    foreign_keys: &mut Vec<ForeignKeySpec>,
    allow_pk: bool,
) -> BlastResult<()> {
    loop {
        let prompt = format!(
            "Columns defined: {}. What would you like to do?",
            columns.len()
        );
        let items = ["Add column", "Continue to next step"];
        let action = Select::with_theme(theme)
            .with_prompt(&prompt)
            .default(0)
            .items(&items)
            .interact()?;

        match action {
            0 => {
                let column = pick_column(theme, allow_pk, foreign_keys)?;
                columns.push(column);
            }
            1 => return Ok(()),
            other => {
                return Err(BlastError::Invalid(format!(
                    "unexpected column action index {}",
                    other
                )));
            }
        }
    }
}

fn pick_column(
    theme: &ColorfulTheme,
    allow_pk: bool,
    foreign_keys: &mut Vec<ForeignKeySpec>,
) -> BlastResult<ColumnSpec> {
    let name = prompt_text(theme, "Enter column name")?;

    let type_index = FuzzySelect::with_theme(theme)
        .with_prompt(format!("Select type for column '{}'", name))
        .items(COLUMN_TYPES)
        .default(0)
        .interact()?;
    let base_type = COLUMN_TYPES[type_index];
    let sql_type = refine_column_type(theme, base_type)?;

    let nullable = Confirm::with_theme(theme)
        .with_prompt("Is this column nullable?")
        .default(false)
        .interact()?;

    let unique = Confirm::with_theme(theme)
        .with_prompt("Is this column unique?")
        .default(false)
        .interact()?;

    let raw_default: String = Input::<String>::with_theme(theme)
        .with_prompt("Enter default value (or leave empty for none)")
        .allow_empty(true)
        .interact_text()?;
    let default = if raw_default.is_empty() {
        None
    } else {
        Some(raw_default)
    };

    let primary_key = if !allow_pk {
        false
    } else if sql_type == "SERIAL" {
        true
    } else {
        Confirm::with_theme(theme)
            .with_prompt("Is this column a primary key?")
            .default(false)
            .interact()?
    };

    let is_foreign_key = Confirm::with_theme(theme)
        .with_prompt("Is this column a foreign key?")
        .default(false)
        .interact()?;

    if is_foreign_key {
        let fk = pick_foreign_key(theme, &name)?;
        match fk {
            Some(spec) => foreign_keys.push(spec),
            None => {}
        }
    }

    Ok(ColumnSpec {
        name,
        sql_type,
        nullable,
        unique,
        default,
        primary_key,
    })
}

fn refine_column_type(theme: &ColorfulTheme, base: &str) -> BlastResult<String> {
    match base {
        "VARCHAR" | "CHAR" => {
            let length = Input::<usize>::with_theme(theme)
                .with_prompt(format!("Enter length for {}", base))
                .default(255)
                .interact_text()?;
            Ok(format!("{}({})", base, length))
        }
        "DECIMAL" | "NUMERIC" => {
            let precision = Input::<usize>::with_theme(theme)
                .with_prompt("Enter precision (total digits)")
                .default(10)
                .interact_text()?;
            let scale = Input::<usize>::with_theme(theme)
                .with_prompt("Enter scale (decimal digits)")
                .default(2)
                .interact_text()?;
            Ok(format!("{}({},{})", base, precision, scale))
        }
        "ARRAY" => {
            let elem_index = FuzzySelect::with_theme(theme)
                .with_prompt("Select the array element type")
                .items(ARRAY_ELEMENT_TYPES)
                .default(0)
                .interact()?;
            let elem = ARRAY_ELEMENT_TYPES[elem_index];
            Ok(format!("{}[]", elem))
        }
        other => Ok(other.to_string()),
    }
}

fn pick_foreign_key(
    theme: &ColorfulTheme,
    column_name: &str,
) -> BlastResult<Option<ForeignKeySpec>> {
    let existing_tables = crate::database::seeds::get_existing_tables();
    if existing_tables.is_empty() {
        return Ok(None);
    }

    let index = Select::with_theme(theme)
        .with_prompt("Select referenced table")
        .items(&existing_tables)
        .default(0)
        .interact()?;
    let referenced_table = existing_tables[index].clone();

    let referenced_column: String = Input::<String>::with_theme(theme)
        .with_prompt("Enter referenced column")
        .default("id".to_string())
        .interact_text()?;

    Ok(Some(ForeignKeySpec {
        column: column_name.to_string(),
        referenced_table,
        referenced_column,
    }))
}

fn prompt_text(theme: &ColorfulTheme, prompt: &str) -> BlastResult<String> {
    let value: String = Input::with_theme(theme)
        .with_prompt(prompt)
        .interact_text()?;
    Ok(value)
}
