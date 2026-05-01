use std::fmt;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::{
    cata_log,
    meltdown::MeltDown,
    structs::list_query::{ListQuery, ListQueryBuilder, ListResponse, ParseError, Sort, SortDirection},
};

pub const DEFAULT_PAGE: u32 = 1;

pub const DEFAULT_PAGE_SIZE: u32 = 25;

pub const MAX_PAGE_SIZE: u32 = 200;

impl fmt::Display for SortDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SortDirection::Asc => f.write_str("asc"),
            SortDirection::Desc => f.write_str("desc"),
        }
    }
}

impl Sort {
    fn parse_segment(seg: &str) -> Result<Self, ParseError> {
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            return Err(ParseError::EmptySortSegment);
        }
        let Some(rest) = trimmed.strip_prefix('-') else {
            Self::validate_column(trimmed)?;
            return Ok(Sort {
                column: trimmed.to_string(),
                direction: SortDirection::Asc,
            });
        };
        if rest.is_empty() {
            return Err(ParseError::EmptySortSegment);
        }
        Self::validate_column(rest)?;
        Ok(Sort {
            column: rest.to_string(),
            direction: SortDirection::Desc,
        })
    }

    fn validate_column(col: &str) -> Result<(), ParseError> {
        if !col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(ParseError::InvalidColumnIdent(col.to_string()));
        }
        Ok(())
    }

    pub fn as_wire(&self) -> String {
        match self.direction {
            SortDirection::Asc => self.column.clone(),
            SortDirection::Desc => format!("-{}", self.column),
        }
    }
}

impl ListQuery {
    pub fn from_query_str(raw: &str) -> Result<Self, MeltDown> {
        let mut builder = ListQueryBuilder::default();
        for (key, value) in iter_pairs(raw) {
            builder.absorb(&key, &value)?;
        }
        builder.build()
    }

    pub fn filter_first(&self, column: &str) -> Option<&str> {
        self.filter.iter().find_map(|(k, v)| (k == column).then_some(v.as_str()))
    }

    pub fn filter_all<'a>(&'a self, column: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.filter.iter().filter_map(move |(k, v)| (k == column).then_some(v.as_str()))
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ListQuery
where
    S: Send + Sync,
{
    type Rejection = MeltDown;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(raw) = parts.uri.query() else {
            return ListQuery::from_query_str("");
        };
        ListQuery::from_query_str(raw)
    }
}

impl ListQueryBuilder {
    fn absorb(&mut self, key: &str, value: &str) -> Result<(), MeltDown> {
        match key {
            "page" => {
                let n: u32 = value
                    .parse()
                    .map_err(|e| bad_request(ParseError::NotAnInt { key: "page", value: value.to_string() }).with_context("parse_error", format!("{}", e)))?;
                if n == 0 {
                    return Err(bad_request(ParseError::PageMustBePositive));
                }
                self.page = Some(n);
            }
            "page_size" => {
                let n: u32 = value.parse().map_err(|e| {
                    bad_request(ParseError::NotAnInt {
                        key: "page_size",
                        value: value.to_string(),
                    })
                    .with_context("parse_error", format!("{}", e))
                })?;
                if n == 0 {
                    return Err(bad_request(ParseError::PageSizeMustBePositive));
                }
                self.page_size = Some(n.min(MAX_PAGE_SIZE));
            }
            "sort" => {
                for seg in value.split(',') {
                    let s = Sort::parse_segment(seg).map_err(bad_request)?;
                    self.sort.push(s);
                }
            }
            other => {
                let Some(col) = parse_filter_key(other)? else {
                    return Err(bad_request(ParseError::UnknownKey(other.to_string())));
                };
                self.filter.push((col, value.to_string()));
            }
        }
        Ok(())
    }

    fn build(self) -> Result<ListQuery, MeltDown> {
        let page = option_or(self.page, DEFAULT_PAGE);
        let page_size = option_or(self.page_size, DEFAULT_PAGE_SIZE);
        Ok(ListQuery {
            page,
            page_size,
            sort: self.sort,
            filter: self.filter,
        })
    }
}

fn option_or<T>(opt: Option<T>, default: T) -> T {
    let Some(v) = opt else {
        return default;
    };
    v
}

fn split_pair(s: &str) -> (&str, &str) {
    let Some((k, v)) = s.split_once('=') else {
        return (s, "");
    };
    (k, v)
}

fn parse_filter_key(key: &str) -> Result<Option<String>, MeltDown> {
    let Some(rest) = key.strip_prefix("filter[") else {
        return Ok(None);
    };
    let Some(col) = rest.strip_suffix(']') else {
        return Err(bad_request(ParseError::MalformedFilterKey(key.to_string())));
    };
    if col.is_empty() {
        return Err(bad_request(ParseError::EmptyFilterColumn));
    }
    if !col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(bad_request(ParseError::InvalidColumnIdent(col.to_string())));
    }
    Ok(Some(col.to_string()))
}

fn iter_pairs(raw: &str) -> impl Iterator<Item = (String, String)> + '_ {
    raw.split('&').filter(|s| !s.is_empty()).map(|pair| {
        let (k, v) = split_pair(pair);
        (decode(k), decode(v))
    })
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b as char);
                        i += 3;
                    }
                    Err(e) => {
                        cata_log!(Debug, format!("decode: malformed %-escape '{}': {}", hex, e));
                        out.push('%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

fn bad_request(e: ParseError) -> MeltDown {
    match e {
        ParseError::NotAnInt { key, value } => MeltDown::bad_request(format!("list query: `{}` must be an unsigned integer", key))
            .with_context("query_key", key)
            .with_context("query_value", value),
        ParseError::PageMustBePositive => MeltDown::bad_request("list query: `page` must be >= 1").with_context("query_key", "page"),
        ParseError::PageSizeMustBePositive => MeltDown::bad_request("list query: `page_size` must be >= 1").with_context("query_key", "page_size"),
        ParseError::EmptySortSegment => MeltDown::bad_request("list query: `sort` segment is empty (use `col` or `-col`, comma-separated)").with_context("query_key", "sort"),
        ParseError::InvalidColumnIdent(col) => MeltDown::bad_request("list query: column identifier must be `[A-Za-z0-9_]+`").with_context("column", col),
        ParseError::MalformedFilterKey(key) => MeltDown::bad_request("list query: filter key must be `filter[col]`").with_context("query_key", key),
        ParseError::EmptyFilterColumn => MeltDown::bad_request("list query: filter column is empty (`filter[]=...`)").with_context("query_key", "filter[]"),
        ParseError::UnknownKey(key) => MeltDown::bad_request(format!("list query: unknown query key `{}` (allowed: page, page_size, sort, filter[<col>])", key)).with_context("query_key", key),
    }
}

impl<T> ListResponse<T> {
    pub fn new(items: Vec<T>, page: u32, page_size: u32, total: u64) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            let ps = page_size as u64;
            total.div_ceil(ps)
        };
        Self {
            items,
            page,
            page_size,
            total,
            total_pages,
        }
    }

    pub fn from_query(items: Vec<T>, query: &ListQuery, total: u64) -> Self {
        Self::new(items, query.page, query.page_size, total)
    }
}

impl<T> IntoResponse for ListResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}
