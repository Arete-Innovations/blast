//! Locked wire schema for list endpoints:
//!
//! ```text
//! ?page=<u32>&page_size=<u32>&sort=[-]<col>[,[-]<col>]*&filter[<col>]=<val>...
//! ```
//!
//! # Design notes (alt — Builder/explicit-parser flavour)
//!
//! This module deliberately avoids `serde_qs` / `serde_urlencoded` derive magic.
//! The query parser walks the raw query string by hand and dispatches per
//! known key, so:
//!
//! 1. Every parse failure produces a `MeltDown::bad_request` with structured
//!    `with_context` entries (key, raw value, reason). No opaque
//!    "missing field `foo`" serde errors.
//! 2. Sort directions are turned into a typed [`SortDirection`] enum at parse
//!    time — downstream code never re-parses the `-` prefix.
//! 3. Filter values keep their wire order in a `Vec<(String, String)>`,
//!    duplicates allowed (lets the consumer treat repeats as OR-semantics).
//! 4. Strict mode (default) rejects unknown query keys — typos in dev fail
//!    loud rather than silently no-op. Solo-dev framework opinion: typos
//!    are bugs, not features.
//! 5. `page_size > MAX_PAGE_SIZE` clamps silently to MAX. `page_size = 0`
//!    is a hard error. `page = 0` is a hard error.

use std::fmt;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::meltdown::MeltDown;

/// Default `page` when the client omits it.
pub const DEFAULT_PAGE: u32 = 1;

/// Default `page_size` when the client omits it.
pub const DEFAULT_PAGE_SIZE: u32 = 25;

/// Hard ceiling on `page_size`. Larger values are silently clamped.
pub const MAX_PAGE_SIZE: u32 = 200;

/// Parsed `sort=[-]<col>` segment.
///
/// `direction` is decided once at parse time; consumers never re-inspect a `-`
/// prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    pub column: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl fmt::Display for SortDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SortDirection::Asc => f.write_str("asc"),
            SortDirection::Desc => f.write_str("desc"),
        }
    }
}

impl Sort {
    /// `-col` -> `Sort{ col, Desc }`, otherwise Asc.
    fn parse_segment(seg: &str) -> Result<Self, ParseError> {
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            return Err(ParseError::EmptySortSegment);
        }
        if let Some(rest) = trimmed.strip_prefix('-') {
            if rest.is_empty() {
                return Err(ParseError::EmptySortSegment);
            }
            Self::validate_column(rest)?;
            Ok(Sort { column: rest.to_string(), direction: SortDirection::Desc })
        } else {
            Self::validate_column(trimmed)?;
            Ok(Sort { column: trimmed.to_string(), direction: SortDirection::Asc })
        }
    }

    fn validate_column(col: &str) -> Result<(), ParseError> {
        if !col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(ParseError::InvalidColumnIdent(col.to_string()));
        }
        Ok(())
    }

    /// Re-emit the canonical wire form (`col` or `-col`).
    pub fn as_wire(&self) -> String {
        match self.direction {
            SortDirection::Asc => self.column.clone(),
            SortDirection::Desc => format!("-{}", self.column),
        }
    }
}

/// Parsed `?page&page_size&sort&filter[col]=val` query string.
///
/// Build via the axum extractor, [`ListQuery::from_query_str`], or the
/// `Default` impl. Field access is plain — no getters — because the wire
/// contract IS the public contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub page: u32,
    pub page_size: u32,
    pub sort: Vec<Sort>,
    /// `(column, value)` pairs, wire order preserved, duplicates kept.
    pub filter: Vec<(String, String)>,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            page_size: DEFAULT_PAGE_SIZE,
            sort: Vec::new(),
            filter: Vec::new(),
        }
    }
}

impl ListQuery {
    /// Parse a raw query string (no leading `?`). Hand-rolled so error
    /// messages can carry structured context.
    pub fn from_query_str(raw: &str) -> Result<Self, MeltDown> {
        let mut builder = ListQueryBuilder::default();
        for (key, value) in iter_pairs(raw) {
            builder.absorb(&key, &value)?;
        }
        builder.build()
    }

    /// First filter value for `column`, if any.
    pub fn filter_first(&self, column: &str) -> Option<&str> {
        self.filter.iter().find_map(|(k, v)| (k == column).then_some(v.as_str()))
    }

    /// All filter values for `column` in wire order.
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
        let raw = parts.uri.query().unwrap_or("");
        ListQuery::from_query_str(raw)
    }
}

// ---------------------------------------------------------------------------
// Builder — explicit per-key dispatch, accumulates structured errors.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ListQueryBuilder {
    page: Option<u32>,
    page_size: Option<u32>,
    sort: Vec<Sort>,
    filter: Vec<(String, String)>,
}

impl ListQueryBuilder {
    fn absorb(&mut self, key: &str, value: &str) -> Result<(), MeltDown> {
        match key {
            "page" => {
                let n: u32 = value.parse().map_err(|_| {
                    bad_request(ParseError::NotAnInt { key: "page", value: value.to_string() })
                })?;
                if n == 0 {
                    return Err(bad_request(ParseError::PageMustBePositive));
                }
                self.page = Some(n);
            }
            "page_size" => {
                let n: u32 = value.parse().map_err(|_| {
                    bad_request(ParseError::NotAnInt {
                        key: "page_size",
                        value: value.to_string(),
                    })
                })?;
                if n == 0 {
                    return Err(bad_request(ParseError::PageSizeMustBePositive));
                }
                self.page_size = Some(n.min(MAX_PAGE_SIZE));
            }
            "sort" => {
                // Either repeated `sort=` params or one comma-separated value.
                for seg in value.split(',') {
                    let s = Sort::parse_segment(seg).map_err(bad_request)?;
                    self.sort.push(s);
                }
            }
            other => {
                if let Some(col) = parse_filter_key(other)? {
                    self.filter.push((col, value.to_string()));
                } else {
                    // Strict mode: unknown keys are bugs.
                    return Err(bad_request(ParseError::UnknownKey(other.to_string())));
                }
            }
        }
        Ok(())
    }

    fn build(self) -> Result<ListQuery, MeltDown> {
        Ok(ListQuery {
            page: self.page.unwrap_or(DEFAULT_PAGE),
            page_size: self.page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            sort: self.sort,
            filter: self.filter,
        })
    }
}

/// Parses `filter[col]` -> Some("col"); anything else -> Ok(None);
/// malformed brackets -> Err.
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

// ---------------------------------------------------------------------------
// Tiny URL-pair iterator. Avoids pulling `url`/`form_urlencoded` as deps.
// Handles `+` -> ` ` and `%xx` decoding for both keys and values.
// ---------------------------------------------------------------------------

fn iter_pairs(raw: &str) -> impl Iterator<Item = (String, String)> + '_ {
    raw.split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| {
            let (k, v) = match pair.split_once('=') {
                Some((k, v)) => (k, v),
                None => (pair, ""),
            };
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
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                } else {
                    // Malformed escape — pass the `%` through verbatim.
                    out.push('%');
                    i += 1;
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

// ---------------------------------------------------------------------------
// Parse error -> MeltDown::bad_request with structured context.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    NotAnInt { key: &'static str, value: String },
    PageMustBePositive,
    PageSizeMustBePositive,
    EmptySortSegment,
    InvalidColumnIdent(String),
    MalformedFilterKey(String),
    EmptyFilterColumn,
    UnknownKey(String),
}

fn bad_request(e: ParseError) -> MeltDown {
    match e {
        ParseError::NotAnInt { key, value } => MeltDown::bad_request(format!(
            "list query: `{}` must be an unsigned integer",
            key
        ))
        .with_context("query_key", key)
        .with_context("query_value", value),
        ParseError::PageMustBePositive => {
            MeltDown::bad_request("list query: `page` must be >= 1").with_context("query_key", "page")
        }
        ParseError::PageSizeMustBePositive => {
            MeltDown::bad_request("list query: `page_size` must be >= 1")
                .with_context("query_key", "page_size")
        }
        ParseError::EmptySortSegment => MeltDown::bad_request(
            "list query: `sort` segment is empty (use `col` or `-col`, comma-separated)",
        )
        .with_context("query_key", "sort"),
        ParseError::InvalidColumnIdent(col) => {
            MeltDown::bad_request("list query: column identifier must be `[A-Za-z0-9_]+`")
                .with_context("column", col)
        }
        ParseError::MalformedFilterKey(key) => {
            MeltDown::bad_request("list query: filter key must be `filter[col]`")
                .with_context("query_key", key)
        }
        ParseError::EmptyFilterColumn => {
            MeltDown::bad_request("list query: filter column is empty (`filter[]=...`)")
                .with_context("query_key", "filter[]")
        }
        ParseError::UnknownKey(key) => {
            MeltDown::bad_request(format!(
                "list query: unknown query key `{}` (allowed: page, page_size, sort, filter[<col>])",
                key
            ))
            .with_context("query_key", key)
        }
    }
}

// ---------------------------------------------------------------------------
// ListResponse<T> — paginated wire envelope.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub total_pages: u64,
}

impl<T> ListResponse<T> {
    /// Auto-computes `total_pages = ceil(total / page_size)`.
    /// `page_size == 0` would be a div-by-zero, but `ListQuery` rejects that
    /// at parse time. We still defend here: 0 -> 0 pages.
    pub fn new(items: Vec<T>, page: u32, page_size: u32, total: u64) -> Self {
        let total_pages = if page_size == 0 {
            0
        } else {
            let ps = page_size as u64;
            total.div_ceil(ps)
        };
        Self { items, page, page_size, total, total_pages }
    }

    /// Convenience: build from the original `ListQuery`.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meltdown::MeltType;

    #[test]
    fn defaults_when_query_empty() {
        let q = ListQuery::from_query_str("").expect("empty query is valid");
        assert_eq!(q.page, DEFAULT_PAGE);
        assert_eq!(q.page_size, DEFAULT_PAGE_SIZE);
        assert!(q.sort.is_empty());
        assert!(q.filter.is_empty());
    }

    #[test]
    fn parses_page_and_page_size() {
        let q = ListQuery::from_query_str("page=3&page_size=50").unwrap();
        assert_eq!(q.page, 3);
        assert_eq!(q.page_size, 50);
    }

    #[test]
    fn page_size_clamps_to_max() {
        let q = ListQuery::from_query_str(&format!("page_size={}", MAX_PAGE_SIZE + 999)).unwrap();
        assert_eq!(q.page_size, MAX_PAGE_SIZE);
    }

    #[test]
    fn page_zero_is_bad_request() {
        let err = ListQuery::from_query_str("page=0").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn page_size_zero_is_bad_request() {
        let err = ListQuery::from_query_str("page_size=0").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn page_must_be_int() {
        let err = ListQuery::from_query_str("page=abc").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn parses_multi_sort_via_repeat_and_comma() {
        let q = ListQuery::from_query_str("sort=name&sort=-created_at,id").unwrap();
        assert_eq!(q.sort.len(), 3);
        assert_eq!(q.sort[0], Sort { column: "name".into(), direction: SortDirection::Asc });
        assert_eq!(q.sort[1], Sort { column: "created_at".into(), direction: SortDirection::Desc });
        assert_eq!(q.sort[2], Sort { column: "id".into(), direction: SortDirection::Asc });
    }

    #[test]
    fn sort_dash_only_is_rejected() {
        let err = ListQuery::from_query_str("sort=-").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn sort_invalid_column_is_rejected() {
        let err = ListQuery::from_query_str("sort=bad-col").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn parses_filters_with_url_encoding_and_keeps_order() {
        // `filter[name]=Jo%20hn&filter[role]=admin&filter[name]=Jane`
        let q = ListQuery::from_query_str(
            "filter%5Bname%5D=Jo%20hn&filter%5Brole%5D=admin&filter%5Bname%5D=Jane",
        )
        .unwrap();
        assert_eq!(q.filter.len(), 3);
        assert_eq!(q.filter[0], ("name".into(), "Jo hn".into()));
        assert_eq!(q.filter[1], ("role".into(), "admin".into()));
        assert_eq!(q.filter[2], ("name".into(), "Jane".into()));
        assert_eq!(q.filter_first("role"), Some("admin"));
        assert_eq!(q.filter_all("name").collect::<Vec<_>>(), vec!["Jo hn", "Jane"]);
    }

    #[test]
    fn malformed_filter_bracket_is_rejected() {
        // `filter[foo` (no closing bracket).
        let err = ListQuery::from_query_str("filter%5Bfoo=x").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn empty_filter_column_is_rejected() {
        let err = ListQuery::from_query_str("filter%5B%5D=x").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn unknown_key_is_strict_rejected() {
        let err = ListQuery::from_query_str("page=1&fancy=true").unwrap_err();
        assert!(err.is(MeltType::BadRequest));
    }

    #[test]
    fn sort_as_wire_round_trips() {
        let s = Sort { column: "created_at".into(), direction: SortDirection::Desc };
        assert_eq!(s.as_wire(), "-created_at");
        let s = Sort { column: "name".into(), direction: SortDirection::Asc };
        assert_eq!(s.as_wire(), "name");
    }

    #[test]
    fn list_response_total_pages_ceil() {
        let r = ListResponse::new(Vec::<i32>::new(), 1, 25, 0);
        assert_eq!(r.total_pages, 0);
        let r = ListResponse::new(Vec::<i32>::new(), 1, 25, 1);
        assert_eq!(r.total_pages, 1);
        let r = ListResponse::new(Vec::<i32>::new(), 1, 25, 25);
        assert_eq!(r.total_pages, 1);
        let r = ListResponse::new(Vec::<i32>::new(), 1, 25, 26);
        assert_eq!(r.total_pages, 2);
        let r = ListResponse::new(Vec::<i32>::new(), 1, 10, 99);
        assert_eq!(r.total_pages, 10);
    }

    #[test]
    fn list_response_from_query() {
        let q = ListQuery { page: 2, page_size: 50, ..Default::default() };
        let r = ListResponse::from_query(vec![1u32, 2, 3], &q, 120);
        assert_eq!(r.page, 2);
        assert_eq!(r.page_size, 50);
        assert_eq!(r.total, 120);
        assert_eq!(r.total_pages, 3);
        assert_eq!(r.items, vec![1u32, 2, 3]);
    }

    #[test]
    fn list_response_serializes_to_expected_shape() {
        let r = ListResponse::new(vec!["a".to_string(), "b".to_string()], 1, 25, 2);
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["items"], serde_json::json!(["a", "b"]));
        assert_eq!(v["page"], 1);
        assert_eq!(v["page_size"], 25);
        assert_eq!(v["total"], 2);
        assert_eq!(v["total_pages"], 1);
    }
}
