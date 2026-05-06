//! Renderer for the `<AdminCrudMenu/>` component.
//!
//! One block per resource at `gen_level >= Pages`. Each block carries
//! direct links to that resource's list / create / first-row pages. The
//! component is hand-rendered as plain anchors (no Leptos `<A/>` import,
//! no router context required) so the admin page can drop it inline.

use crate::{codegen::structs::naming::type_stem_for_resource, state::{ResourceState, Verb}};

#[derive(Debug, Clone)]
pub struct ResourceMenuEntry {
    pub table: String,
    pub stem: String,
    pub has_list: bool,
    pub has_create: bool,
    pub has_get: bool,
    pub has_update: bool,
}

pub fn collect_entries(resources: &[ResourceState]) -> Vec<ResourceMenuEntry> {
    let mut out: Vec<ResourceMenuEntry> = Vec::new();
    for r in resources {
        let table = r.name.as_str().to_string();
        let stem = type_stem_for_resource(r);
        out.push(ResourceMenuEntry {
            table,
            stem,
            has_list: emit_html(r, Verb::List),
            has_create: emit_html(r, Verb::Create),
            has_get: emit_html(r, Verb::Get),
            has_update: emit_html(r, Verb::Update),
        });
    }
    out.sort_by(|a, b| a.table.cmp(&b.table));
    out
}

fn emit_html(r: &ResourceState, v: Verb) -> bool {
    match r.verbs.get(&v) {
        Some(state) => state.emit_html_page,
        None => false, // allow: absent verb declaration → no HTML page emission for this verb
    }
}

pub fn render_admin_crud_menu(entries: &[ResourceMenuEntry]) -> String {
    let mut out = String::new();
    out.push_str("use ::leptos::prelude::*;\n\n");
    out.push_str("/// Auto-emitted admin shortcut menu — one card per resource at\n");
    out.push_str("/// gen_level >= Pages, with direct links to its CRUD pages.\n");
    out.push_str("#[component]\n");
    out.push_str("pub fn AdminCrudMenu() -> impl IntoView {\n");

    if entries.is_empty() {
        out.push_str("    view! {\n");
        out.push_str("        <div class=\"admin-crud-menu admin-crud-menu--empty\">\n");
        out.push_str("            <p>\"No resources at gen_level >= Pages yet — run \"<code>\"blast migration\"</code>\" to scaffold one.\"</p>\n");
        out.push_str("        </div>\n");
        out.push_str("    }\n");
        out.push_str("}\n");
        return out;
    }

    out.push_str("    view! {\n");
    out.push_str("        <ul class=\"admin-crud-menu\">\n");
    for entry in entries {
        push_entry(&mut out, entry);
    }
    out.push_str("        </ul>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn push_entry(out: &mut String, entry: &ResourceMenuEntry) {
    let table = &entry.table;
    let stem = &entry.stem;
    out.push_str("            <li class=\"admin-crud-menu__resource\">\n");
    out.push_str(&format!("                <h3 class=\"admin-crud-menu__title\">\"{stem}\"</h3>\n"));
    out.push_str("                <ul class=\"admin-crud-menu__actions\">\n");
    if entry.has_list {
        out.push_str(&format!(
            "                    <li><a class=\"admin-crud-menu__link\" href=\"/{table}\">\"List\"</a></li>\n"
        ));
    }
    if entry.has_create {
        out.push_str(&format!(
            "                    <li><a class=\"admin-crud-menu__link\" href=\"/{table}/new\">\"+ New\"</a></li>\n"
        ));
    }
    if entry.has_get && !entry.has_list {
        // List is the canonical entry; only surface a bare detail hint when there's no list page.
        out.push_str(&format!(
            "                    <li><span class=\"admin-crud-menu__link admin-crud-menu__link--hint\">\"Detail at /{table}/:id\"</span></li>\n"
        ));
    }
    if entry.has_update && !entry.has_get && !entry.has_list {
        out.push_str(&format!(
            "                    <li><span class=\"admin-crud-menu__link admin-crud-menu__link--hint\">\"Edit at /{table}/:id/edit\"</span></li>\n"
        ));
    }
    out.push_str("                </ul>\n");
    out.push_str("            </li>\n");
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;

    use super::*;
    use crate::state::{
        CrankPolicy,
        ResourceName,
        SqlType,
        names::{FieldName,
    },
        resource::{AuthMode, FieldState, FieldVariant, ListOptions, RESOURCE_SCHEMA_VERSION, VerbState},
        AppState, GenLevel, ResourceState,
    };

    fn resource(table: &str, verbs: &[Verb]) -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: [FieldVariant::Db, FieldVariant::Public].into_iter().collect(),
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            
            kind: Default::default(),
        },
        );
        let mut verb_map: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in verbs {
            verb_map.insert(
                *v,
                VerbState {
                    auth: AuthMode::AuthRequired,
                    list_options: matches!(v, Verb::List).then(|| ListOptions {
                        paginated: true,
                        filterable_columns: BTreeMap::new(),
                        sortable_columns: BTreeSet::new(),
                        default_sort: None,
                        max_page_size: None,
                    }),
                    emit_rest_api: true,
                    emit_html_page: true,
                                    crank_policy: CrankPolicy::None,
                },
            );
        }
        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new(table),
            fields,
            verbs: verb_map,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: GenLevel::Pages,
            list_layout: None,
            detail_layout: None,
            toggle_endpoint: None,
            live_topics: Vec::new(),
        }
    }

    #[test]
    fn empty_emits_friendly_placeholder() {
        let body = render_admin_crud_menu(&[]);
        assert!(body.contains("admin-crud-menu--empty"));
        assert!(body.contains("blast migration"));
        assert!(body.contains("pub fn AdminCrudMenu"));
    }

    #[test]
    fn lists_resource_with_list_and_create_links() {
        let r = resource("postari", &[Verb::List, Verb::Get, Verb::Create]);
        let entries = collect_entries(&[r]);
        let body = render_admin_crud_menu(&entries);
        assert!(body.contains("href=\"/postari\""), "list link: {body}");
        assert!(body.contains("href=\"/postari/new\""), "create link: {body}");
        assert!(body.contains("\"Postari\""), "stem heading: {body}");
    }

    #[test]
    fn skips_links_for_unselected_verbs() {
        let r = resource("widgets", &[Verb::List, Verb::Get]);
        let entries = collect_entries(&[r]);
        let body = render_admin_crud_menu(&entries);
        assert!(body.contains("href=\"/widgets\""));
        assert!(!body.contains("/widgets/new"), "no create link when verb absent:\n{body}");
    }

    #[test]
    fn entries_sorted_by_table_name() {
        let r1 = resource("zebras", &[Verb::List]);
        let r2 = resource("apples", &[Verb::List]);
        let entries = collect_entries(&[r1, r2]);
        assert_eq!(entries[0].table, "apples");
        assert_eq!(entries[1].table, "zebras");
    }

    #[test]
    fn detail_hint_only_when_no_list() {
        let r = resource("widgets", &[Verb::Get]);
        let entries = collect_entries(&[r]);
        let body = render_admin_crud_menu(&entries);
        assert!(body.contains("Detail at /widgets/:id"), "detail hint when list absent:\n{body}");

        let r2 = resource("widgets", &[Verb::List, Verb::Get]);
        let entries2 = collect_entries(&[r2]);
        let body2 = render_admin_crud_menu(&entries2);
        assert!(!body2.contains("Detail at"), "no hint when list exists:\n{body2}");
    }

    fn _silence_unused() -> AppState {
        AppState::new()
    }
}
