use crate::database::write_migration;
use crate::error::{BlastError, BlastResult};
use crate::io::{Progress, Sink, SinkExt};
use crate::logger;
use crate::schema_parser;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, MultiSelect};
use std::path::{Path, PathBuf};

// ── public surface ────────────────────────────────────────────────────────────

pub struct ColumnArgs {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub is_primary_key: bool,
    pub fk_table: Option<String>,
    pub fk_column: Option<String>,
}

pub struct IndexArgs {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

pub struct Args {
    pub table_name: String,
    pub columns: Vec<ColumnArgs>,
    pub indexes: Vec<IndexArgs>,
    pub include_timestamps: bool,
}

pub struct Outcome {
    pub table_name: String,
    pub up_sql_path: PathBuf,
    pub down_sql_path: PathBuf,
    pub column_count: usize,
}

pub fn pick_args(project_root: &Path) -> BlastResult<Args> {
    let theme = ColorfulTheme::default();

    let table_name = prompt_table_name(&theme)?;

    let mut raw_cols: Vec<ColumnDef> = Vec::new();
    raw_cols.push(ColumnDef::primary_key());

    loop {
        let prompt_msg = format!(
            "Columns so far: {}. Add another column?",
            describe_columns(&raw_cols)
        );
        let add_more = Confirm::with_theme(&theme)
            .with_prompt(prompt_msg)
            .default(true)
            .interact()?;
        if !add_more {
            break;
        }

        match collect_column(&theme, &table_name, project_root) {
            Ok(col) => raw_cols.push(col),
            Err(e) => {
                logger::warning(&format!("column skipped: {}", e))?;
            }
        }
    }

    let include_timestamps = Confirm::with_theme(&theme)
        .with_prompt("Auto-add created_at / updated_at TIMESTAMPTZ NOT NULL DEFAULT now()?")
        .default(true)
        .interact()?;

    if include_timestamps {
        raw_cols.push(ColumnDef::timestamp("created_at"));
        raw_cols.push(ColumnDef::timestamp("updated_at"));
    }

    let raw_indexes = prompt_indexes(&theme, &table_name, &raw_cols)?;

    let columns: Vec<ColumnArgs> = raw_cols
        .into_iter()
        .map(|c| ColumnArgs {
            name: c.name,
            sql_type: c.sql_type,
            nullable: c.nullable,
            default: c.default,
            is_primary_key: c.is_primary_key,
            fk_table: c.fk.as_ref().map(|f| f.table.clone()),
            fk_column: c.fk.map(|f| f.column),
        })
        .collect();

    let indexes: Vec<IndexArgs> = raw_indexes
        .into_iter()
        .map(|i| IndexArgs {
            name: i.name,
            columns: i.columns,
            unique: i.unique,
        })
        .collect();

    Ok(Args {
        table_name,
        columns,
        indexes,
        include_timestamps,
    })
}

pub fn run(
    args: Args,
    sink: &mut dyn Sink,
    _progress: &mut dyn Progress,
) -> BlastResult<Outcome> {
    let col_defs = args_to_col_defs(&args);
    let idx_defs = args_to_idx_defs(&args);

    let up_sql = render_up_sql(&args.table_name, &col_defs, &idx_defs);
    let down_sql = render_down_sql(&args.table_name, &idx_defs);

    let migration_name = format!("create_{}", args.table_name);
    let dir = write_migration(&migration_name, &up_sql, &down_sql)?;
    logger::success(&format!("Migration written: {}", dir.display()))?;
    sink.success(format!("Migration written: {}", dir.display()));

    let column_count = args.columns.len();
    let up_sql_path = dir.join("up.sql");
    let down_sql_path = dir.join("down.sql");

    Ok(Outcome {
        table_name: args.table_name,
        up_sql_path,
        down_sql_path,
        column_count,
    })
}

pub fn run_with_picker(
    project_root: &Path,
    sink: &mut dyn Sink,
    progress: &mut dyn Progress,
) -> BlastResult<Outcome> {
    let theme = ColorfulTheme::default();

    let args = pick_args(project_root)?;

    let col_defs = args_to_col_defs(&args);
    let idx_defs = args_to_idx_defs(&args);
    let up_sql = render_up_sql(&args.table_name, &col_defs, &idx_defs);
    let down_sql = render_down_sql(&args.table_name, &idx_defs);

    logger::info("\n=== Generated migration preview ===")?;
    logger::info(&format!("up.sql:\n{}", up_sql))?;
    logger::info(&format!("down.sql:\n{}", down_sql))?;
    logger::info("===================================\n")?;

    let confirmed = Confirm::with_theme(&theme)
        .with_prompt("Write this migration?")
        .default(true)
        .interact()?;

    if !confirmed {
        logger::info("Migration discarded; nothing written.")?;
        sink.info("Migration discarded; nothing written.");
        return Err(BlastError::Invalid("migration cancelled by user".to_string()));
    }

    run(args, sink, progress)
}

// ── private helpers ───────────────────────────────────────────────────────────

struct ColumnDef {
    name: String,
    sql_type: String,
    nullable: bool,
    default: Option<String>,
    is_primary_key: bool,
    fk: Option<ForeignKey>,
}

struct ForeignKey {
    table: String,
    column: String,
}

impl ColumnDef {
    fn primary_key() -> Self {
        ColumnDef {
            name: "id".to_string(),
            sql_type: "BIGSERIAL".to_string(),
            nullable: false,
            default: None,
            is_primary_key: true,
            fk: None,
        }
    }

    fn timestamp(name: &str) -> Self {
        ColumnDef {
            name: name.to_string(),
            sql_type: "TIMESTAMPTZ".to_string(),
            nullable: false,
            default: Some("now()".to_string()),
            is_primary_key: false,
            fk: None,
        }
    }
}

struct IndexDef {
    name: String,
    columns: Vec<String>,
    unique: bool,
}

fn fk_from_args(fk_table: &Option<String>, fk_column: &Option<String>) -> Option<ForeignKey> {
    match (fk_table, fk_column) {
        (Some(t), Some(col)) => Some(ForeignKey {
            table: t.clone(),
            column: col.clone(),
        }),
        (Some(_t), None) => None,
        (None, Some(_col)) => None,
        (None, None) => None,
    }
}

fn args_to_col_defs(args: &Args) -> Vec<ColumnDef> {
    args.columns
        .iter()
        .map(|c| ColumnDef {
            name: c.name.clone(),
            sql_type: c.sql_type.clone(),
            nullable: c.nullable,
            default: c.default.clone(),
            is_primary_key: c.is_primary_key,
            fk: fk_from_args(&c.fk_table, &c.fk_column),
        })
        .collect()
}

fn args_to_idx_defs(args: &Args) -> Vec<IndexDef> {
    args.indexes
        .iter()
        .map(|i| IndexDef {
            name: i.name.clone(),
            columns: i.columns.clone(),
            unique: i.unique,
        })
        .collect()
}

fn describe_columns(columns: &[ColumnDef]) -> String {
    columns
        .iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

fn prompt_table_name(theme: &ColorfulTheme) -> BlastResult<String> {
    let raw: String = Input::with_theme(theme)
        .with_prompt("Table name (snake_case)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if is_snake_case(input) {
                Ok(())
            } else {
                Err("must match ^[a-z][a-z0-9_]*$")
            }
        })
        .interact_text()?;
    Ok(raw)
}

fn is_snake_case(s: &str) -> bool {
    let trimmed = s.trim();
    let first = match trimmed.chars().next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

const COLUMN_TYPES: &[&str] = &[
    "BIGSERIAL PK",
    "BIGINT",
    "INTEGER",
    "TEXT",
    "TEXT NOT NULL",
    "BOOLEAN",
    "TIMESTAMPTZ",
    "JSONB",
    "UUID",
    "BYTEA",
    "NUMERIC(p,s)",
];

fn collect_column(
    theme: &ColorfulTheme,
    current_table: &str,
    project_root: &Path,
) -> BlastResult<ColumnDef> {
    let name: String = Input::with_theme(theme)
        .with_prompt("Column name (snake_case)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if is_snake_case(input) {
                Ok(())
            } else {
                Err("must match ^[a-z][a-z0-9_]*$")
            }
        })
        .interact_text()?;

    let type_idx = FuzzySelect::with_theme(theme)
        .with_prompt(format!("Type for '{}'", name))
        .items(COLUMN_TYPES)
        .default(3)
        .interact()?;
    let type_label = COLUMN_TYPES[type_idx];

    let mut sql_type;
    let mut forced_not_null = false;
    let mut is_pk = false;

    match type_label {
        "BIGSERIAL PK" => {
            sql_type = "BIGSERIAL".to_string();
            is_pk = true;
            forced_not_null = true;
        }
        "TEXT NOT NULL" => {
            sql_type = "TEXT".to_string();
            forced_not_null = true;
        }
        "NUMERIC(p,s)" => {
            let precision: u32 = Input::with_theme(theme)
                .with_prompt("Precision (total digits)")
                .default(12)
                .interact_text()?;
            let scale: u32 = Input::with_theme(theme)
                .with_prompt("Scale (decimal digits)")
                .default(2)
                .interact_text()?;
            sql_type = format!("NUMERIC({},{})", precision, scale);
        }
        other => {
            sql_type = other.to_string();
        }
    }

    let nullable = if forced_not_null {
        false
    } else {
        Confirm::with_theme(theme)
            .with_prompt(format!("Is '{}' nullable?", name))
            .default(false)
            .interact()?
    };

    let default_raw: String = Input::with_theme(theme)
        .with_prompt(format!("Default for '{}' (leave empty for none)", name))
        .allow_empty(true)
        .interact_text()?;
    let default = if default_raw.trim().is_empty() {
        None
    } else {
        Some(default_raw.trim().to_string())
    };

    let want_fk = !is_pk
        && Confirm::with_theme(theme)
            .with_prompt(format!("Add foreign key for '{}'?", name))
            .default(false)
            .interact()?;

    let fk = if want_fk {
        pick_fk_target(theme, current_table, project_root)?
    } else {
        None
    };

    if sql_type.as_str() == "BIGSERIAL" && !is_pk {
        sql_type = "BIGSERIAL".to_string();
    }

    Ok(ColumnDef {
        name,
        sql_type,
        nullable,
        default,
        is_primary_key: is_pk,
        fk,
    })
}

fn pick_fk_target(
    theme: &ColorfulTheme,
    current_table: &str,
    project_root: &Path,
) -> BlastResult<Option<ForeignKey>> {
    let tables = discover_tables(current_table, project_root);
    if tables.is_empty() {
        logger::warning(
            "No tables found in src/database/schema.rs or migrations/. Skipping FK.",
        )?;
        return Ok(None);
    }

    let target_idx = FuzzySelect::with_theme(theme)
        .with_prompt("Referenced table")
        .items(&tables)
        .default(0)
        .interact()?;
    let target_table = tables[target_idx].clone();

    let target_column: String = Input::with_theme(theme)
        .with_prompt("Referenced column")
        .default("id".to_string())
        .interact_text()?;

    Ok(Some(ForeignKey {
        table: target_table,
        column: target_column,
    }))
}

fn discover_tables(current_table: &str, project_root: &Path) -> Vec<String> {
    let schema_path = project_root.join("src/database/schema.rs");
    let mut tables: Vec<String> = Vec::new();

    if schema_path.exists() {
        match schema_parser::parse_schema(&schema_path) {
            Ok(parsed) => {
                for t in parsed {
                    tables.push(t.name);
                }
            }
            Err(e) => {
                drop(e);
            }
        }
    }

    if tables.is_empty() {
        tables = crate::database::seeds::get_existing_tables();
    }

    tables.retain(|t| t != current_table);
    tables.sort();
    tables.dedup();
    tables
}

fn prompt_indexes(
    theme: &ColorfulTheme,
    table_name: &str,
    columns: &[ColumnDef],
) -> BlastResult<Vec<IndexDef>> {
    let mut indexes: Vec<IndexDef> = Vec::new();

    let want = Confirm::with_theme(theme)
        .with_prompt("Create any indexes?")
        .default(false)
        .interact()?;
    if !want {
        return Ok(indexes);
    }

    let candidate_names: Vec<String> = columns
        .iter()
        .filter(|c| !c.is_primary_key)
        .map(|c| c.name.clone())
        .collect();
    if candidate_names.is_empty() {
        logger::warning("No non-PK columns available for indexing.")?;
        return Ok(indexes);
    }

    loop {
        let picks = MultiSelect::with_theme(theme)
            .with_prompt("Select columns for the next index (space to toggle, enter to confirm)")
            .items(&candidate_names)
            .interact()?;

        if picks.is_empty() {
            logger::info("No columns selected — skipping index.")?;
        } else {
            let cols: Vec<String> = picks
                .into_iter()
                .map(|i| candidate_names[i].clone())
                .collect();
            let suggested_name = format!("idx_{}_{}", table_name, cols.join("_"));
            let name: String = Input::with_theme(theme)
                .with_prompt("Index name")
                .default(suggested_name)
                .interact_text()?;

            let unique = Confirm::with_theme(theme)
                .with_prompt("Unique index?")
                .default(false)
                .interact()?;

            indexes.push(IndexDef {
                name,
                columns: cols,
                unique,
            });
        }

        let add_more = Confirm::with_theme(theme)
            .with_prompt("Add another index?")
            .default(false)
            .interact()?;
        if !add_more {
            break;
        }
    }

    Ok(indexes)
}

fn render_up_sql(table: &str, columns: &[ColumnDef], indexes: &[IndexDef]) -> String {
    let mut out = String::new();
    out.push_str(&format!("CREATE TABLE {} (\n", table));

    let mut lines: Vec<String> = Vec::new();
    for c in columns {
        let mut line = format!("    {} {}", c.name, c.sql_type);
        if !c.nullable {
            line.push_str(" NOT NULL");
        }
        match &c.default {
            Some(d) => line.push_str(&format!(" DEFAULT {}", d)),
            None => {}
        }
        if c.is_primary_key {
            line.push_str(" PRIMARY KEY");
        }
        match &c.fk {
            Some(fk) => line.push_str(&format!(" REFERENCES {}({})", fk.table, fk.column)),
            None => {}
        }
        lines.push(line);
    }

    out.push_str(&lines.join(",\n"));
    out.push_str("\n);\n");

    for idx in indexes {
        let unique_kw = if idx.unique { "UNIQUE " } else { "" };
        out.push_str(&format!(
            "CREATE {}INDEX {} ON {} ({});\n",
            unique_kw,
            idx.name,
            table,
            idx.columns.join(", ")
        ));
    }

    out
}

fn render_down_sql(table: &str, indexes: &[IndexDef]) -> String {
    let mut out = String::new();
    for idx in indexes {
        out.push_str(&format!("DROP INDEX IF EXISTS {};\n", idx.name));
    }
    out.push_str(&format!("DROP TABLE IF EXISTS {};\n", table));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str, nullable: bool) -> ColumnDef {
        ColumnDef {
            name: name.to_string(),
            sql_type: ty.to_string(),
            nullable,
            default: None,
            is_primary_key: false,
            fk: None,
        }
    }

    #[test]
    fn snake_case_validator() {
        assert!(is_snake_case("users"));
        assert!(is_snake_case("user_roles_v2"));
        assert!(!is_snake_case(""));
        assert!(!is_snake_case("Users"));
        assert!(!is_snake_case("1table"));
        assert!(!is_snake_case("has space"));
    }

    #[test]
    fn renders_up_sql_with_pk_and_fk() {
        let cols = vec![
            ColumnDef::primary_key(),
            ColumnDef {
                name: "user_id".to_string(),
                sql_type: "BIGINT".to_string(),
                nullable: false,
                default: None,
                is_primary_key: false,
                fk: Some(ForeignKey {
                    table: "users".to_string(),
                    column: "id".to_string(),
                }),
            },
            col("note", "TEXT", true),
        ];
        let sql = render_up_sql("widgets", &cols, &[]);
        assert!(sql.contains("CREATE TABLE widgets ("));
        assert!(sql.contains("id BIGSERIAL NOT NULL PRIMARY KEY"));
        assert!(sql.contains("user_id BIGINT NOT NULL REFERENCES users(id)"));
        assert!(sql.contains("note TEXT"));
    }

    #[test]
    fn renders_indexes() {
        let cols = vec![ColumnDef::primary_key(), col("email", "TEXT", false)];
        let idx = IndexDef {
            name: "idx_users_email".to_string(),
            columns: vec!["email".to_string()],
            unique: true,
        };
        let sql = render_up_sql("users", &cols, &[idx]);
        assert!(sql.contains("CREATE UNIQUE INDEX idx_users_email ON users (email);"));
    }

    #[test]
    fn renders_down_sql_drops_indexes_first() {
        let idx = IndexDef {
            name: "idx_x".to_string(),
            columns: vec!["a".to_string()],
            unique: false,
        };
        let sql = render_down_sql("widgets", &[idx]);
        let drop_idx_pos = sql.find("DROP INDEX IF EXISTS idx_x").expect("idx drop");
        let drop_table_pos = sql.find("DROP TABLE IF EXISTS widgets").expect("table drop");
        assert!(drop_idx_pos < drop_table_pos);
    }

    #[test]
    fn timestamp_columns_have_now_default() {
        let c = ColumnDef::timestamp("created_at");
        assert_eq!(c.sql_type, "TIMESTAMPTZ");
        assert!(!c.nullable);
        match c.default {
            Some(v) => assert_eq!(v, "now()"),
            None => panic!("expected default"),
        }
    }
}
