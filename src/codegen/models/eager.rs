//! Eager-loader scope emission per declared relation.
//!
//! Per locked design eager loading is opt-in only. Each emitted accessor
//! flips a boolean on the query state; the executor performs the JOIN +
//! projection only when the flag is set. The default load returns the
//! base row.

/// Forward-compat shape mirrored from the eventual upstream Relation
/// definition. Kept local so this branch compiles in isolation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    /// User-facing accessor name; appears in the with-rel-name accessor.
    pub rel_name: String,
    /// Foreign-key column on this resource pointing at the related table.
    pub fk_column: String,
    /// Related table name (snake_case plural).
    pub target_table: String,
}

/// Emit one scope method per relation. Methods land inside the query
/// builder impl block; the caller wraps them.
pub fn emit_methods(out: &mut String, relations: &[Relation]) {
    for r in relations {
        let rel = &r.rel_name;
        let body = format!(
            r#"    /// Eager-load the `{rel}` relation alongside the base row.
    pub fn with_{rel}(mut self) -> Self {{
        self.with_{rel} = true;
        self
    }}
"#,
        );
        out.push_str(&body);
    }
}

/// Emit the per-relation boolean fields for the query builder struct.
pub fn emit_struct_fields(out: &mut String, relations: &[Relation]) {
    for r in relations {
        let line = format!("    pub(crate) with_{rel}: bool,\n", rel = r.rel_name);
        out.push_str(&line);
    }
}

/// Emit the per-relation initializers used inside the constructor.
pub fn emit_struct_init(out: &mut String, relations: &[Relation]) {
    for r in relations {
        let line = format!("            with_{rel}: false,\n", rel = r.rel_name);
        out.push_str(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(name: &str, fk: &str, target: &str) -> Relation {
        Relation {
            rel_name: name.to_string(),
            fk_column: fk.to_string(),
            target_table: target.to_string(),
        }
    }

    #[test]
    fn emit_methods_one_per_relation() {
        let mut out = String::new();
        emit_methods(
            &mut out,
            &[
                rel("author", "author_id", "users"),
                rel("category", "category_id", "categories"),
            ],
        );
        assert!(out.contains("pub fn with_author(mut self) -> Self"));
        assert!(out.contains("pub fn with_category(mut self) -> Self"));
        assert!(out.contains("self.with_author = true"));
        assert!(out.contains("self.with_category = true"));
    }

    #[test]
    fn emit_methods_no_relations_yields_empty() {
        let mut out = String::new();
        emit_methods(&mut out, &[]);
        assert!(out.is_empty());
    }

    #[test]
    fn emit_struct_fields_renders_bool_flags() {
        let mut out = String::new();
        emit_struct_fields(&mut out, &[rel("author", "author_id", "users")]);
        assert!(out.contains("pub(crate) with_author: bool,"));
    }

    #[test]
    fn emit_struct_init_renders_default_false() {
        let mut out = String::new();
        emit_struct_init(&mut out, &[rel("author", "author_id", "users")]);
        assert!(out.contains("with_author: false,"));
    }
}
