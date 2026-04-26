
use std::fmt::Write as _;

use crate::admin::schema_view::AdminTable;

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

const STYLE: &str = "<style>\
body{font-family:system-ui,sans-serif;margin:1.5rem;color:#222;background:#fafafa;}\
h1,h2{margin-top:0;}a{color:#06c;}table{border-collapse:collapse;width:100%;}\
th,td{border:1px solid #ccc;padding:.4rem .6rem;text-align:left;font-size:.9rem;}\
th{background:#eee;}form{margin:.5rem 0;}label{display:block;margin:.4rem 0;}\
input,textarea{width:100%;font-family:monospace;}.bar{margin:1rem 0;}\
.muted{color:#888;font-size:.85rem;}.warn{color:#a00;}\
</style>";

fn shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>{STYLE}</head>\
         <body><nav class=\"bar\"><a href=\"/admin/\">[admin home]</a></nav>{body}</body></html>",
        title = escape(title),
    )
}

pub fn index_page(tables: &[AdminTable]) -> String {
    let mut body = String::from("<h1>Catalyst admin</h1>");
    if tables.is_empty() {
        body.push_str(
            "<p class=\"muted\">No tables registered. Pass an <code>AdminConfig</code> to \
             <code>admin_router_with</code> or run <code>blast gen primer</code> to populate \
             <code>target/primer/*.json</code>.</p>",
        );
        return shell("admin", &body);
    }
    body.push_str("<ul>");
    for t in tables {
        let _ = write!(
            body,
            "<li><a href=\"/admin/{name}/\">{label}</a> <span class=\"muted\">({name})</span></li>",
            name = escape(&t.name),
            label = escape(&t.display_name),
        );
    }
    body.push_str("</ul>");
    shell("admin", &body)
}

pub fn list_page(
    table: &AdminTable,
    columns: &[String],
    rows: &[Vec<String>],
    page: i64,
    page_size: i64,
    sort_col: &str,
) -> String {
    let mut body = format!(
        "<h1>{}</h1><div class=\"bar\"><a href=\"/admin/{name}/new\">new</a> \
         &middot; sorted by <code>{sort}</code></div>",
        escape(&table.display_name),
        name = escape(&table.name),
        sort = escape(sort_col),
    );

    body.push_str("<table><thead><tr>");
    for c in columns {
        let _ = write!(body, "<th>{}</th>", escape(c));
    }
    body.push_str("<th></th></tr></thead><tbody>");

    if rows.is_empty() {
        let _ = write!(
            body,
            "<tr><td colspan=\"{}\" class=\"muted\">no rows</td></tr>",
            columns.len() + 1
        );
    } else {
        for row in rows {
            body.push_str("<tr>");
            for cell in row {
                let _ = write!(body, "<td>{}</td>", escape(cell));
            }
            let pk = row.first().map(String::as_str).unwrap_or("");
            let _ = write!(
                body,
                "<td><a href=\"/admin/{name}/{pk}\">view</a></td></tr>",
                name = escape(&table.name),
                pk = escape(pk),
            );
        }
    }
    body.push_str("</tbody></table>");

    let prev = (page - 1).max(1);
    let next = page + 1;
    let _ = write!(
        body,
        "<div class=\"bar\">\
         <a href=\"/admin/{name}/?page={prev}&page_size={ps}\">&lt; prev</a> \
         page {page} \
         <a href=\"/admin/{name}/?page={next}&page_size={ps}\">next &gt;</a></div>",
        name = escape(&table.name),
        ps = page_size,
    );

    shell(&format!("admin :: {}", table.display_name), &body)
}

pub fn new_form(table: &AdminTable) -> String {
    let mut body = format!(
        "<h1>New {}</h1><form method=\"post\" action=\"/admin/{name}/\">",
        escape(&table.display_name),
        name = escape(&table.name),
    );
    for c in &table.columns {
        if c.name == "id" {
            continue;
        }
        let _ = write!(
            body,
            "<label>{label}<br><input name=\"{n}\"></label>",
            label = escape(&c.name),
            n = escape(&c.name),
        );
    }
    body.push_str("<button type=\"submit\">create</button></form>");
    shell(&format!("admin :: new {}", table.display_name), &body)
}

pub fn detail_page(table: &AdminTable, columns: &[String], values: &[String], pk: &str) -> String {
    let mut body = format!(
        "<h1>{} :: {}</h1>",
        escape(&table.display_name),
        escape(pk),
    );
    body.push_str("<table>");
    for (c, v) in columns.iter().zip(values.iter()) {
        let _ = write!(body, "<tr><th>{}</th><td>{}</td></tr>", escape(c), escape(v));
    }
    body.push_str("</table>");

    let _ = write!(
        body,
        "<h2>edit</h2><form method=\"post\" action=\"/admin/{name}/{pk}/edit\">",
        name = escape(&table.name),
        pk = escape(pk),
    );
    for (c, v) in columns.iter().zip(values.iter()) {
        if c == "id" {
            continue;
        }
        let _ = write!(
            body,
            "<label>{label}<br><input name=\"{n}\" value=\"{val}\"></label>",
            label = escape(c),
            n = escape(c),
            val = escape(v),
        );
    }
    body.push_str("<button type=\"submit\">save</button></form>");

    let _ = write!(
        body,
        "<h2 class=\"warn\">delete</h2>\
         <form method=\"post\" action=\"/admin/{name}/{pk}/delete\">\
         <button type=\"submit\">delete this row</button></form>",
        name = escape(&table.name),
        pk = escape(pk),
    );

    shell(&format!("admin :: {} {}", table.display_name, pk), &body)
}

pub fn not_found(message: &str) -> String {
    shell("admin :: not found", &format!("<h1>not found</h1><p>{}</p>", escape(message)))
}
