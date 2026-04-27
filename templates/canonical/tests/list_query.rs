use canonical::{
    meltdown::MeltType,
    structs::list_query::{ListQuery, ListResponse, Sort, SortDirection},
    transport::http::list_query::{DEFAULT_PAGE, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE},
};

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
    assert_eq!(
        q.sort[0],
        Sort {
            column: "name".into(),
            direction: SortDirection::Asc
        }
    );
    assert_eq!(
        q.sort[1],
        Sort {
            column: "created_at".into(),
            direction: SortDirection::Desc
        }
    );
    assert_eq!(
        q.sort[2],
        Sort {
            column: "id".into(),
            direction: SortDirection::Asc
        }
    );
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
    let q = ListQuery::from_query_str("filter%5Bname%5D=Jo%20hn&filter%5Brole%5D=admin&filter%5Bname%5D=Jane").unwrap();
    assert_eq!(q.filter.len(), 3);
    assert_eq!(q.filter[0], ("name".into(), "Jo hn".into()));
    assert_eq!(q.filter[1], ("role".into(), "admin".into()));
    assert_eq!(q.filter[2], ("name".into(), "Jane".into()));
    assert_eq!(q.filter_first("role"), Some("admin"));
    assert_eq!(q.filter_all("name").collect::<Vec<_>>(), vec!["Jo hn", "Jane"]);
}

#[test]
fn malformed_filter_bracket_is_rejected() {
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
    let s = Sort {
        column: "created_at".into(),
        direction: SortDirection::Desc,
    };
    assert_eq!(s.as_wire(), "-created_at");
    let s = Sort {
        column: "name".into(),
        direction: SortDirection::Asc,
    };
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
    let q = ListQuery {
        page: 2,
        page_size: 50,
        ..Default::default()
    };
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
