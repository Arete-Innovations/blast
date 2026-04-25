use crate::database::migration_wizard::spec::{
    AlterTableSpec, ColumnSpec, ForeignKeySpec, NewTableSpec,
};

pub fn render_new_table_up(spec: &NewTableSpec) -> String {
    let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", spec.table);
    let total_lines = spec.columns.len() + spec.foreign_keys.len();
    let mut emitted = 0usize;

    for col in &spec.columns {
        sql.push_str("    ");
        sql.push_str(&render_column_def(col));
        emitted += 1;
        if emitted < total_lines {
            sql.push_str(",\n");
        } else {
            sql.push('\n');
        }
    }

    for fk in &spec.foreign_keys {
        sql.push_str("    ");
        sql.push_str(&render_inline_fk(fk));
        emitted += 1;
        if emitted < total_lines {
            sql.push_str(",\n");
        } else {
            sql.push('\n');
        }
    }

    sql.push_str(");\n");
    sql
}

pub fn render_new_table_down(spec: &NewTableSpec) -> String {
    format!("DROP TABLE {};\n", spec.table)
}

pub fn render_alter_table_up(spec: &AlterTableSpec) -> String {
    let mut sql = format!("ALTER TABLE {} ", spec.table);
    let total_clauses = spec.additions.len() + spec.foreign_keys.len();
    let mut emitted = 0usize;

    for col in &spec.additions {
        sql.push_str("ADD COLUMN ");
        sql.push_str(&render_column_def_no_pk(col));
        emitted += 1;
        if emitted < total_clauses {
            sql.push_str(", ");
        }
    }

    for fk in &spec.foreign_keys {
        sql.push_str(&format!(
            "ADD FOREIGN KEY ({}) REFERENCES {}({})",
            fk.column, fk.referenced_table, fk.referenced_column
        ));
        emitted += 1;
        if emitted < total_clauses {
            sql.push_str(", ");
        }
    }

    sql.push_str(";\n");
    sql
}

pub fn render_alter_table_down(_spec: &AlterTableSpec) -> String {
    "-- reverse changes here\n".to_string()
}

fn render_column_def(col: &ColumnSpec) -> String {
    let nullable_part = if col.nullable { "" } else { " NOT NULL" };
    let unique_part = if col.unique { " UNIQUE" } else { "" };
    let default_part = render_default(&col.default);
    let pk_part = if col.primary_key { " PRIMARY KEY" } else { "" };
    format!(
        "{} {}{}{}{}{}",
        col.name, col.sql_type, nullable_part, unique_part, default_part, pk_part
    )
}

fn render_column_def_no_pk(col: &ColumnSpec) -> String {
    let nullable_part = if col.nullable { "" } else { " NOT NULL" };
    let unique_part = if col.unique { " UNIQUE" } else { "" };
    let default_part = render_default(&col.default);
    format!(
        "{} {}{}{}{}",
        col.name, col.sql_type, nullable_part, unique_part, default_part
    )
}

fn render_default(default: &Option<String>) -> String {
    let mut out = String::new();
    match default {
        Some(d) => {
            if !d.is_empty() {
                out.push_str(" DEFAULT ");
                out.push_str(d);
            }
        }
        None => {}
    }
    out
}

fn render_inline_fk(fk: &ForeignKeySpec) -> String {
    format!(
        "FOREIGN KEY ({}) REFERENCES {}({})",
        fk.column, fk.referenced_table, fk.referenced_column
    )
}

pub fn custom_up_template() -> String {
    "-- Write your custom SQL migration here\n-- Example: ALTER TABLE table_name ADD COLUMN column_name TYPE;\n".to_string()
}

pub fn custom_down_template() -> String {
    "-- Write how to reverse the changes here\n-- Example: ALTER TABLE table_name DROP COLUMN column_name;\n".to_string()
}
