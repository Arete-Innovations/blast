use std::path::Path;

use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Item, Token, Type,
};

use crate::error::{BlastError, BlastResult};

pub struct ParsedTable {
    pub name: String,
    pub primary_key: Vec<String>,
    pub columns: Vec<ParsedColumn>,
}

pub struct ParsedColumn {
    pub name: String,
    pub diesel_type: String,
    pub nullable: bool,
}

pub fn parse_schema(path: &Path) -> BlastResult<Vec<ParsedTable>> {
    let content = std::fs::read_to_string(path)?;
    let file = syn::parse_file(&content).map_err(|e| BlastError::Invalid(format!("schema parse {}: {}", path.display(), e)))?;

    let mut tables: Vec<ParsedTable> = Vec::new();

    for item in &file.items {
        let macro_item = match item {
            Item::Macro(m) => m,
            _other => continue,
        };

        let last_seg = match macro_item.mac.path.segments.last() {
            Some(s) => s,
            None => continue,
        };

        if last_seg.ident != "table" {
            continue;
        }

        let table = parse_table_body(path, macro_item.mac.tokens.clone())?;
        tables.push(table);
    }

    Ok(tables)
}

fn parse_table_body(path: &Path, tokens: proc_macro2::TokenStream) -> BlastResult<ParsedTable> {
    let parser = |input: ParseStream| -> syn::Result<ParsedTable> { parse_table_tokens(input) };

    syn::parse::Parser::parse2(parser, tokens).map_err(|e| BlastError::Invalid(format!("unexpected table! shape in {}: {}", path.display(), e)))
}

fn parse_table_tokens(input: ParseStream) -> syn::Result<ParsedTable> {
    let name_ident: syn::Ident = input.parse()?;
    let name = name_ident.to_string();

    let primary_key = if input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in input);
        let pk_list: Punctuated<syn::Ident, Token![,]> = content.parse_terminated(syn::Ident::parse, Token![,])?;
        pk_list.into_iter().map(|i| i.to_string()).collect()
    } else {
        Vec::new()
    };

    let body;
    syn::braced!(body in input);

    let columns = parse_columns(&body)?;

    Ok(ParsedTable { name, primary_key, columns })
}

fn parse_columns(input: ParseStream) -> syn::Result<Vec<ParsedColumn>> {
    let mut columns: Vec<ParsedColumn> = Vec::new();

    while !input.is_empty() {
        while input.peek(Token![#]) {
            let _attr: syn::Attribute = input.call(syn::Attribute::parse_outer)?.remove(0);
        }

        if input.is_empty() {
            break;
        }

        let col_name: syn::Ident = input.parse()?;
        let _arrow: Token![->] = input.parse()?;
        let col_type: Type = input.parse()?;
        let _comma: Token![,] = input.parse()?;

        let (diesel_type, nullable) = extract_type_info(&col_type)?;

        columns.push(ParsedColumn {
            name: col_name.to_string(),
            diesel_type,
            nullable,
        });
    }

    Ok(columns)
}

fn extract_type_info(ty: &Type) -> syn::Result<(String, bool)> {
    let path = match ty {
        Type::Path(p) => &p.path,
        _other => {
            return Err(syn::Error::new_spanned(ty, "expected a type path"));
        }
    };

    let seg = match path.segments.last() {
        Some(s) => s,
        None => {
            return Err(syn::Error::new_spanned(ty, "empty type path"));
        }
    };

    if seg.ident == "Nullable" {
        let inner = extract_nullable_inner(&seg.arguments)?;
        return Ok((inner, true));
    }

    Ok((seg.ident.to_string(), false))
}

fn extract_nullable_inner(args: &syn::PathArguments) -> syn::Result<String> {
    let angle_args = match args {
        syn::PathArguments::AngleBracketed(a) => a,
        _other => {
            return Err(syn::Error::new(proc_macro2::Span::call_site(), "Nullable must have angle-bracketed generic args"));
        }
    };

    let first_arg = match angle_args.args.first() {
        Some(a) => a,
        None => {
            return Err(syn::Error::new(proc_macro2::Span::call_site(), "Nullable<> has no inner type"));
        }
    };

    let inner_ty = match first_arg {
        syn::GenericArgument::Type(t) => t,
        _other => {
            return Err(syn::Error::new(proc_macro2::Span::call_site(), "Nullable inner arg must be a type"));
        }
    };

    let inner_path = match inner_ty {
        Type::Path(p) => &p.path,
        _other => {
            return Err(syn::Error::new_spanned(inner_ty, "expected a type path inside Nullable"));
        }
    };

    let inner_seg = match inner_path.segments.last() {
        Some(s) => s,
        None => {
            return Err(syn::Error::new_spanned(inner_ty, "empty type path inside Nullable"));
        }
    };

    Ok(inner_seg.ident.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn temp_schema(content: &str) -> std::io::Result<tempfile::NamedTempFile> {
        let mut f = tempfile::NamedTempFile::new()?;
        f.write_all(content.as_bytes())?;
        Ok(f)
    }

    #[test]
    fn parses_single_pk_with_nullable_columns() {
        let src = r#"
            diesel::table! {
                users (id) {
                    id -> Int4,
                    email -> Varchar,
                    username -> Nullable<Varchar>,
                    created_at -> Timestamptz,
                    deleted_at -> Nullable<Timestamptz>,
                }
            }
        "#;
        let f = temp_schema(src).expect("temp file");
        let tables = parse_schema(f.path()).expect("parse");
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.name, "users");
        assert_eq!(t.primary_key, vec!["id"]);
        assert_eq!(t.columns.len(), 5);

        let email = t.columns.iter().find(|c| c.name == "email").expect("email");
        assert_eq!(email.diesel_type, "Varchar");
        assert!(!email.nullable);

        let username = t.columns.iter().find(|c| c.name == "username").expect("username");
        assert_eq!(username.diesel_type, "Varchar");
        assert!(username.nullable);

        let deleted_at = t.columns.iter().find(|c| c.name == "deleted_at").expect("deleted_at");
        assert_eq!(deleted_at.diesel_type, "Timestamptz");
        assert!(deleted_at.nullable);
    }

    #[test]
    fn parses_composite_pk_table() {
        let src = r#"
            diesel::table! {
                user_roles (user_id, role_id) {
                    user_id -> Int4,
                    role_id -> Int4,
                    granted_at -> Timestamptz,
                }
            }
        "#;
        let f = temp_schema(src).expect("temp file");
        let tables = parse_schema(f.path()).expect("parse");
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.name, "user_roles");
        assert_eq!(t.primary_key, vec!["user_id", "role_id"]);
        assert_eq!(t.columns.len(), 3);
    }

    #[test]
    fn skips_joinable_macros_without_error() {
        let src = r#"
            diesel::table! {
                sessions (id) {
                    id -> Int4,
                    user_id -> Int4,
                    token_hash -> Bytea,
                }
            }
            diesel::joinable!(sessions -> users (user_id));
            diesel::allow_tables_to_appear_in_same_query!(sessions, users,);
        "#;
        let f = temp_schema(src).expect("temp file");
        let tables = parse_schema(f.path()).expect("parse");
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "sessions");
    }

    #[test]
    fn returns_invalid_on_garbage_input() {
        let src = "this is not valid rust @@@";
        let f = temp_schema(src).expect("temp file");
        let result = parse_schema(f.path());
        assert!(result.is_err());
    }

    #[test]
    fn parses_real_catalyst_schema() {
        let path = std::path::Path::new("/home/tragdate/codumeu/catablast/catalyst/src/database/schema.rs");
        let tables = parse_schema(path).expect("parse catalyst schema");
        assert!(!tables.is_empty(), "expected at least one table");
        for t in &tables {
            assert!(!t.name.is_empty());
            assert!(!t.columns.is_empty());
        }
        let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"users"), "expected users table");
        assert!(names.contains(&"sessions"), "expected sessions table");
    }

    #[test]
    fn parses_column_with_max_length_attr() {
        let src = r#"
            diesel::table! {
                posts (id) {
                    id -> Int4,
                    #[max_length = 100]
                    title -> Varchar,
                    content -> Text,
                }
            }
        "#;
        let f = temp_schema(src).expect("temp file");
        let tables = parse_schema(f.path()).expect("parse");
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.columns.len(), 3);
        let title = t.columns.iter().find(|c| c.name == "title").expect("title");
        assert_eq!(title.diesel_type, "Varchar");
        assert!(!title.nullable);
    }
}
