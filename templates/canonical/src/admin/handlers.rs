
use std::sync::Arc;

use axum::{
    extract::{Extension, Path, Query},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_extra::extract::Form;
use diesel::sql_types::{Integer, Jsonb, Text};
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    admin::{schema_view::AdminTable, templates, AdminConfig},
    cata_log,
    database::db::establish_connection,
    meltdown::*,
};

#[derive(diesel::QueryableByName, Debug)]
struct JsonRow {
    #[diesel(sql_type = Jsonb)]
    row: Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListParams {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    page_size: Option<i64>,
}

const MAX_PAGE_SIZE: i64 = 200;

fn lookup_table<'a>(cfg: &'a AdminConfig, name: &str) -> Result<&'a AdminTable, MeltDown> {
    cfg.table(name)
        .ok_or_else(|| MeltDown::new(MeltType::NotFound, format!("admin table `{}`", name)))
}

fn columns_for_render(t: &AdminTable) -> Vec<String> {
    if t.list_columns.is_empty() {
        t.columns.iter().map(|c| c.name.clone()).collect()
    } else {
        t.list_columns.clone()
    }
}

fn cell_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

pub async fn index(Extension(cfg): Extension<Arc<AdminConfig>>) -> Html<String> {
    Html(templates::index_page(&cfg.tables))
}

pub async fn list(
    Extension(cfg): Extension<Arc<AdminConfig>>,
    Path(table): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, MeltDown> {
    let t = lookup_table(&cfg, &table)?;
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
    let offset = (page - 1) * page_size;

    let mut conn = establish_connection().await?;
    let sql = format!(
        "SELECT to_jsonb(t.*) AS row FROM \"{table}\" t ORDER BY id LIMIT $1 OFFSET $2",
        table = t.name,
    );
    let rows: Vec<JsonRow> = diesel::sql_query(sql)
        .bind::<diesel::sql_types::BigInt, _>(page_size)
        .bind::<diesel::sql_types::BigInt, _>(offset)
        .load(&mut conn)
        .await
        .map_err(|e| {
            cata_log!(Error, format!("admin list `{}`: {}", t.name, e));
            MeltDown::from(e)
        })?;

    let render_cols = columns_for_render(t);
    let row_strings: Vec<Vec<String>> = rows
        .iter()
        .map(|r| project_row(&r.row, &render_cols))
        .collect();

    Ok(Html(templates::list_page(t, &render_cols, &row_strings, page, page_size, "id")))
}

pub async fn new_form(
    Extension(cfg): Extension<Arc<AdminConfig>>,
    Path(table): Path<String>,
) -> Result<Html<String>, MeltDown> {
    let t = lookup_table(&cfg, &table)?;
    Ok(Html(templates::new_form(t)))
}

pub async fn detail(
    Extension(cfg): Extension<Arc<AdminConfig>>,
    Path((table, id)): Path<(String, i32)>,
) -> Result<Html<String>, MeltDown> {
    let t = lookup_table(&cfg, &table)?;
    let row = fetch_row(&t.name, id).await?.ok_or_else(|| {
        MeltDown::new(MeltType::RecordNotFound, format!("{}#{}", t.name, id))
    })?;

    let cols: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
    let values = project_row(&row, &cols);
    Ok(Html(templates::detail_page(t, &cols, &values, &id.to_string())))
}

pub async fn create(
    Extension(cfg): Extension<Arc<AdminConfig>>,
    Path(table): Path<String>,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response, MeltDown> {
    let t = lookup_table(&cfg, &table)?;
    let payload = form_to_json(&form, t)?;
    let payload_text = serde_json::to_string(&payload).map_err(|e| {
        MeltDown::new(MeltType::SerializationFailed, format!("encode admin form: {}", e))
    })?;

    let mut conn = establish_connection().await?;
    let sql = format!(
        "INSERT INTO \"{table}\" SELECT * FROM jsonb_populate_record(NULL::\"{table}\", $1::jsonb)",
        table = t.name,
    );
    diesel::sql_query(sql)
        .bind::<Text, _>(payload_text)
        .execute(&mut conn)
        .await
        .map_err(|e| {
            cata_log!(Error, format!("admin insert `{}`: {}", t.name, e));
            MeltDown::from(e)
        })?;

    Ok(Redirect::to(&format!("/admin/{}/", t.name)).into_response())
}

pub async fn update(
    Extension(cfg): Extension<Arc<AdminConfig>>,
    Path((table, id)): Path<(String, i32)>,
    Form(form): Form<Vec<(String, String)>>,
) -> Result<Response, MeltDown> {
    let t = lookup_table(&cfg, &table)?;
    let payload = form_to_json(&form, t)?;
    let payload_text = serde_json::to_string(&payload).map_err(|e| {
        MeltDown::new(MeltType::SerializationFailed, format!("encode admin form: {}", e))
    })?;

    let mut conn = establish_connection().await?;
    let sql = format!(
        "UPDATE \"{table}\" AS dst SET \
         (\"{table}\") = (SELECT row(p.*)::\"{table}\" FROM jsonb_populate_record(dst.*, $1::jsonb) p) \
         WHERE dst.id = $2",
        table = t.name,
    );
    diesel::sql_query(sql)
        .bind::<Text, _>(payload_text)
        .bind::<Integer, _>(id)
        .execute(&mut conn)
        .await
        .map_err(|e| {
            cata_log!(Error, format!("admin update `{}#{}`: {}", t.name, id, e));
            MeltDown::from(e)
        })?;

    Ok(Redirect::to(&format!("/admin/{}/{}", t.name, id)).into_response())
}

pub async fn delete(
    Extension(cfg): Extension<Arc<AdminConfig>>,
    Path((table, id)): Path<(String, i32)>,
) -> Result<Response, MeltDown> {
    let t = lookup_table(&cfg, &table)?;

    let mut conn = establish_connection().await?;
    let sql = format!("DELETE FROM \"{table}\" WHERE id = $1", table = t.name);
    diesel::sql_query(sql)
        .bind::<Integer, _>(id)
        .execute(&mut conn)
        .await
        .map_err(|e| {
            cata_log!(Error, format!("admin delete `{}#{}`: {}", t.name, id, e));
            MeltDown::from(e)
        })?;

    Ok(Redirect::to(&format!("/admin/{}/", t.name)).into_response())
}

async fn fetch_row(table: &str, id: i32) -> Result<Option<Value>, MeltDown> {
    let mut conn = establish_connection().await?;
    let sql = format!(
        "SELECT to_jsonb(t.*) AS row FROM \"{table}\" t WHERE id = $1",
        table = table,
    );
    let mut rows: Vec<JsonRow> = diesel::sql_query(sql)
        .bind::<Integer, _>(id)
        .load(&mut conn)
        .await
        .map_err(|e| {
            cata_log!(Error, format!("admin fetch `{}#{}`: {}", table, id, e));
            MeltDown::from(e)
        })?;
    Ok(rows.pop().map(|r| r.row))
}

fn project_row(row: &Value, columns: &[String]) -> Vec<String> {
    let obj = row.as_object();
    columns
        .iter()
        .map(|c| {
            obj.and_then(|m| m.get(c))
                .map(cell_to_string)
                .unwrap_or_default()
        })
        .collect()
}

fn form_to_json(pairs: &[(String, String)], t: &AdminTable) -> Result<Value, MeltDown> {
    let mut allowed: std::collections::HashSet<&str> =
        t.columns.iter().map(|c| c.name.as_str()).collect();
    allowed.insert("id");

    let mut obj = Map::new();
    for (k, v) in pairs {
        if !allowed.contains(k.as_str()) {
            return Err(MeltDown::new(
                MeltType::ValidationFailed,
                format!("unknown column `{}` for table `{}`", k, t.name),
            ));
        }
        if v.is_empty() {
            obj.insert(k.clone(), Value::Null);
        } else {
            obj.insert(k.clone(), Value::String(v.clone()));
        }
    }
    Ok(Value::Object(obj))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin::schema_view::AdminColumn;

    fn t() -> AdminTable {
        AdminTable {
            name: "users".into(),
            display_name: "Users".into(),
            columns: vec![
                AdminColumn { name: "id".into(), public: true },
                AdminColumn { name: "email".into(), public: true },
            ],
            list_columns: vec![],
        }
    }

    #[test]
    fn form_to_json_filters_unknown_keys() {
        let table = t();
        let err = form_to_json(
            &[("email".into(), "a@b".into()), ("ghost".into(), "x".into())],
            &table,
        )
        .unwrap_err();
        assert_eq!(err.melt_type, MeltType::ValidationFailed);
    }

    #[test]
    fn form_to_json_empty_becomes_null() {
        let table = t();
        let v = form_to_json(&[("email".into(), "".into())], &table).unwrap();
        assert_eq!(v["email"], Value::Null);
    }

    #[test]
    fn project_row_handles_missing_keys() {
        let row = serde_json::json!({"id": 7, "email": "x"});
        let cols = vec!["id".to_string(), "missing".to_string()];
        let out = project_row(&row, &cols);
        assert_eq!(out, vec!["7".to_string(), "".to_string()]);
    }
}
