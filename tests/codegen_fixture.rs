
use std::fs;
use std::path::Path;


fn write_fixture_ir(project_root: &Path) {
    let dir = project_root.join("target").join("primer");
    fs::create_dir_all(&dir).expect("create primer dir");

    let ir = serde_json::json!({
        "table": "users",
        "fields": [
            {
                "name": "id",
                "variants": ["DB", "Public", "Admin"],
                "validation": {}
            },
            {
                "name": "email",
                "variants": ["DB", "Insertable", "Public"],
                "validation": {
                    "validators": [
                        {"Required": null},
                        "Email",
                        {"MaxLen": 255}
                    ]
                }
            }
        ],
        "verbs": [
            {
                "kind": "List",
                "auth": "AuthRequired",
                "filter": {
                    "paginated": true,
                    "filterable_columns": ["role", "created_at"],
                    "sortable_columns": ["created_at", "email"],
                    "default_sort": "-created_at",
                    "max_page_size": 100
                }
            }
        ]
    });

    let path = dir.join("users.json");
    fs::write(&path, serde_json::to_string_pretty(&ir).expect("ser ir"))
        .expect("write users.json");
}

#[test]
fn run_frontend_emits_expected_artifacts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path();
    write_fixture_ir(project_root);

    blast::codegen::run_frontend(project_root).expect("codegen run");

    let validators = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("validators")
        .join("users.ts");
    let validators_src = fs::read_to_string(&validators).expect("read validators ts");
    assert!(
        validators_src.contains("validate_email"),
        "missing validate_email export: {validators_src}"
    );
    assert!(
        validators_src.contains("(v?.length ?? 0) <= 255"),
        "missing MaxLen(255) emission: {validators_src}"
    );
    assert!(
        validators_src.contains("invalid email"),
        "missing Email message: {validators_src}"
    );
    assert!(
        validators_src.contains("required"),
        "missing Required predicate: {validators_src}"
    );

    let validators_index = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("validators")
        .join("index.ts");
    let index_src = fs::read_to_string(&validators_index).expect("read validator index");
    assert!(index_src.contains("./users"));

    let list_query = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("list_query.ts");
    let list_query_src = fs::read_to_string(&list_query).expect("read list_query ts");
    assert!(list_query_src.contains("interface ListQuery"));
    assert!(list_query_src.contains("buildListQuery"));
    assert!(list_query_src.contains("parseListQuery"));

    let resource_list = project_root
        .join("frontend")
        .join("src")
        .join("generated")
        .join("queries")
        .join("users_list.ts");
    let resource_list_src = fs::read_to_string(&resource_list).expect("read users_list ts");
    assert!(resource_list_src.contains("'created_at'"));
    assert!(resource_list_src.contains("'email'"));
    assert!(resource_list_src.contains("'role'"));
    assert!(resource_list_src.contains("'-created_at'"));
    assert!(
        resource_list_src.contains("MAX_PAGE_SIZE: number = 100"),
        "missing per-resource max_page_size cap: {resource_list_src}"
    );
}
