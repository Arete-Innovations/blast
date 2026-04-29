//! TypeScript API client rendering for per-resource fetch functions.
//!
//! Function names must match exactly what `composables_v2/render.rs` imports:
//!   list<Plural>   (imported as `list<Singular>s` in render.rs but resolves to same)
//!   get<Singular>
//!   create<Singular>
//!   update<Singular>
//!   delete<Singular>
//!
//! From composables_v2/render.rs api_alias():
//!   "listResource"  => format!("list{}s as apiList", suffix)   // suffix = singular pascal
//!   "getResource"   => format!("get{} as apiGet", suffix)
//!   "createResource"=> format!("create{} as apiCreate", suffix)
//!   "updateResource"=> format!("update{} as apiUpdate", suffix)
//!   "deleteResource"=> format!("delete{} as apiDelete", suffix)
//!
//! So if resource = "users", singular = "User":
//!   import { listUsers as apiList, getUser as apiGet, ... } from '@/generated/api/users'
//!
//! This module must export: listUsers, getUser, createUser, updateUser, deleteUser
//!
//! The list endpoint returns `ListResponse<T>` (paginated), matching the
//! backend's `ListResponse` type from `catalyst::transport::http::list_query`.

use crate::{
    codegen::structs::naming::type_stem_for_resource,
    state::{ResourceState, Verb},
};

/// Build the full TS API client file body for one resource.
pub fn build_resource_api(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let singular = type_stem_for_resource(resource);
    let plural = plural_of_pascal(&singular, table);

    let has_list = resource.verbs.contains_key(&Verb::List);
    let has_get = resource.verbs.contains_key(&Verb::Get);
    let has_create = resource.verbs.contains_key(&Verb::Create);
    let has_update = resource.verbs.contains_key(&Verb::Update);
    let has_delete = resource.verbs.contains_key(&Verb::Delete);

    let mut imports: Vec<String> = Vec::new();
    if has_list {
        imports.push(format!("type {singular}Public", singular = singular));
    } else if has_get || has_create || has_update {
        imports.push(format!("type {singular}Public", singular = singular));
    }
    if has_create {
        imports.push(format!("type {singular}Insertable", singular = singular));
    }
    if has_update {
        imports.push(format!("type {singular}Patch", singular = singular));
    }

    let mut out = String::new();

    if !imports.is_empty() {
        // Deduplicate (list + get might both need Public)
        let mut seen: Vec<String> = Vec::new();
        for item in imports {
            if !seen.contains(&item) {
                seen.push(item);
            }
        }
        out.push_str(&format!("import {{ {names} }} from '@/generated/types/{table}'\n", names = seen.join(", "), table = table,));
    }

    out.push_str("import type { MeltDownResponse } from '@/generated/types/meltdown'\n");
    out.push('\n');

    // Helpers (auth header) — emitted inline since each api file is standalone.
    out.push_str("function auth_header(): Record<string, string> {\n");
    out.push_str("  const token = localStorage.getItem('token')\n");
    out.push_str("  if (token === null) {\n");
    out.push_str("    return {}\n");
    out.push_str("  }\n");
    out.push_str("  return { Authorization: `Bearer ${token}` }\n");
    out.push_str("}\n\n");

    // Result type alias used by all functions.
    out.push_str("type ApiResult<T> = Promise<{ data: T | null; error: MeltDownResponse | null }>\n\n");

    // list function
    if has_list {
        out.push_str(&render_list_fn(table, &singular, &plural));
        out.push('\n');
    }

    // get function
    if has_get {
        out.push_str(&render_get_fn(table, &singular));
        out.push('\n');
    }

    // create function
    if has_create {
        out.push_str(&render_create_fn(table, &singular));
        out.push('\n');
    }

    // update function
    if has_update {
        out.push_str(&render_update_fn(table, &singular));
        out.push('\n');
    }

    // delete function
    if has_delete {
        out.push_str(&render_delete_fn(table, &singular));
        out.push('\n');
    }

    let trimmed = out.trim_end_matches('\n');
    format!("{trimmed}\n")
}

/// `listUsers` — matches `list{}s as apiList` alias in composables_v2.
/// Returns the items array directly (unwrapping the ListResponse wrapper)
/// plus the pagination envelope's metadata as sibling fields so generated
/// composables can populate `total` / `total_pages` refs without a second
/// roundtrip. Existing pages that destructure `result.data` and
/// `result.error` keep working — the extra fields are additive.
fn render_list_fn(table: &str, singular: &str, plural: &str) -> String {
    format!(
        "interface {singular}ListEnvelope {{\nitems: {singular}Public[]\ntotal: number\ntotal_pages: number\npage: number\npage_size: number\n}}\n\nexport interface {singular}ListResult {{\ndata: {singular}Public[] | \
         null\nerror: MeltDownResponse | null\ntotal: number\ntotal_pages: number\npage: number\npage_size: number\n}}\n\nexport async function list{plural}(\nparams: {{ page?: number; page_size?: number; sort?: string \
         | null; filter?: {{ [key: string]: string | number | boolean | null | undefined }} | null }},\nsignal?: AbortSignal,\n): Promise<{singular}ListResult> {{\nconst url = new URL(`/api/{table}/`, \
         window.location.origin)\nif (params.page !== undefined) {{\nurl.searchParams.set('page', String(params.page))\n}}\nif (params.page_size !== undefined) {{\nurl.searchParams.set('page_size', \
         String(params.page_size))\n}}\nif (params.sort !== undefined && params.sort !== null) {{\nurl.searchParams.set('sort', params.sort)\n}}\nif (params.filter !== null && params.filter !== undefined) {{\nfor \
         (const [key, val] of Object.entries(params.filter)) {{\nif (val === null || val === undefined) {{\ncontinue\n}}\nconst serialized = typeof val === 'object' ? JSON.stringify(val) : \
         String(val)\nurl.searchParams.set(`filter[${{key}}]`, serialized)\n}}\n}}\nconst path = url.pathname + url.search\ntry {{\nconst res = await fetch(path, {{\nheaders: {{ ...auth_header(), Accept: \
         'application/json' }},\nsignal,\n}})\nif (!res.ok) {{\nconst err = (await res.json()) as MeltDownResponse\nreturn {{ data: null, error: err, total: 0, total_pages: 0, page: 0, page_size: 0 }}\n}}\nconst \
         envelope = (await res.json()) as {singular}ListEnvelope\nreturn {{ data: envelope.items, error: null, total: envelope.total, total_pages: envelope.total_pages, page: envelope.page, page_size: \
         envelope.page_size }}\n}} catch (e) {{\nif (e instanceof DOMException && e.name === 'AbortError') {{\nreturn {{ data: null, error: null, total: 0, total_pages: 0, page: 0, page_size: 0 }}\n}}\nconst err: \
         MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network error', context: null }} }}\nreturn {{ data: null, error: err, total: 0, total_pages: 0, page: 0, page_size: 0 }}\n}}\n}}\n",
        table = table,
        singular = singular,
        plural = plural,
    )
}

/// `getUser` — matches `get{} as apiGet` alias.
fn render_get_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function get{singular}(\nid: number,\nsignal?: AbortSignal,\n): ApiResult<{singular}Public> {{\ntry {{\nconst res = await fetch(`/api/{table}/${{id}}`, {{\nheaders: {{ ...auth_header(), Accept: \
         'application/json' }},\nsignal,\n}})\nif (!res.ok) {{\nconst err = (await res.json()) as MeltDownResponse\nreturn {{ data: null, error: err }}\n}}\nconst data = (await res.json()) as {singular}Public\nreturn \
         {{ data, error: null }}\n}} catch (e) {{\nif (e instanceof DOMException && e.name === 'AbortError') {{\nreturn {{ data: null, error: null }}\n}}\nconst err: MeltDownResponse = {{ error: {{ code: 0, type: \
         'NetworkError', message: 'Network error', context: null }} }}\nreturn {{ data: null, error: err }}\n}}\n}}\n",
        table = table,
        singular = singular,
    )
}

/// `createUser` — matches `create{} as apiCreate` alias.
fn render_create_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function create{singular}(\nbody: {singular}Insertable,\n): ApiResult<{singular}Public> {{\ntry {{\nconst res = await fetch(`/api/{table}/`, {{\nmethod: 'POST',\nheaders: {{ ...auth_header(), \
         'Content-Type': 'application/json', Accept: 'application/json' }},\nbody: JSON.stringify(body),\n}})\nif (!res.ok) {{\nconst err = (await res.json()) as MeltDownResponse\nreturn {{ data: null, error: err \
         }}\n}}\nconst data = (await res.json()) as {singular}Public\nreturn {{ data, error: null }}\n}} catch (e) {{\nconst err: MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network \
         error', context: null }} }}\nreturn {{ data: null, error: err }}\n}}\n}}\n",
        table = table,
        singular = singular,
    )
}

/// `updateUser` — matches `update{} as apiUpdate` alias.
fn render_update_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function update{singular}(\nid: number,\npatch: {singular}Patch,\n): ApiResult<{singular}Public> {{\ntry {{\nconst res = await fetch(`/api/{table}/${{id}}`, {{\nmethod: 'PATCH',\nheaders: {{ \
         ...auth_header(), 'Content-Type': 'application/json', Accept: 'application/json' }},\nbody: JSON.stringify(patch),\n}})\nif (!res.ok) {{\nconst err = (await res.json()) as MeltDownResponse\nreturn {{ data: \
         null, error: err }}\n}}\nconst data = (await res.json()) as {singular}Public\nreturn {{ data, error: null }}\n}} catch (e) {{\nconst err: MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', \
         message: 'Network error', context: null }} }}\nreturn {{ data: null, error: err }}\n}}\n}}\n",
        table = table,
        singular = singular,
    )
}

/// `deleteUser` — matches `delete{} as apiDelete` alias.
fn render_delete_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function delete{singular}(\nid: number,\n): ApiResult<{{ id: number }}> {{\ntry {{\nconst res = await fetch(`/api/{table}/${{id}}`, {{\nmethod: 'DELETE',\nheaders: {{ ...auth_header(), Accept: \
         'application/json' }},\n}})\nif (!res.ok) {{\nconst err = (await res.json()) as MeltDownResponse\nreturn {{ data: null, error: err }}\n}}\nreturn {{ data: {{ id }}, error: null }}\n}} catch (e) {{\nconst err: \
         MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network error', context: null }} }}\nreturn {{ data: null, error: err }}\n}}\n}}\n",
        table = table,
        singular = singular,
    )
}

/// Build the plural Pascal form: `User` → `Users`.
/// We use the raw table name (already plural) + PascalCase it, then
/// append to singular stem to keep the composable contract aligned.
/// E.g. for `users`: singular=`User`, plural=`Users`.
/// For `user_accounts`: singular=`UserAccount`, plural=`UserAccounts`.
fn plural_of_pascal(singular: &str, table: &str) -> String {
    // table name is plural snake_case; PascalCase it directly.
    let mut out = String::with_capacity(table.len());
    let mut upper_next = true;
    for ch in table.chars() {
        if ch == '_' || ch == '-' {
            upper_next = true;
            continue;
        }
        if upper_next {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    // If the PascalCased table name starts with the singular, use it as-is.
    // Otherwise fall back to singular + "s".
    if out.starts_with(singular) {
        out
    } else {
        format!("{singular}s")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;

    use super::*;
    use crate::state::{
        names::{FieldName, ResourceName},
        resource::{AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION},
        SqlType,
    };

    fn synth_resource_all_verbs() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();
        let id_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public].into_iter().collect();

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: id_v,
                nullable: false,
                primary_key: true,
                validators: BTreeSet::new(),
            },
        );
        fields.insert(
            FieldName::new("email"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_v,
                nullable: false,
                primary_key: false,
                validators: BTreeSet::new(),
            },
        );

        let mut verbs: IndexMap<Verb, VerbState> = IndexMap::new();
        for v in [Verb::List, Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
            let list_opts = match v {
                Verb::List => Some(ListOptions {
                    paginated: true,
                    filterable_columns: BTreeMap::new(),
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: None,
                }),
                _other => None,
            };
            verbs.insert(
                v,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: list_opts,
                },
            );
        }

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("users"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
            gen_level: crate::state::GenLevel::default(),
        }
    }

    #[test]
    fn exports_all_verb_functions() {
        let r = synth_resource_all_verbs();
        let body = build_resource_api(&r);
        assert!(body.contains("export async function listUsers("), "listUsers missing");
        assert!(body.contains("export async function getUser("), "getUser missing");
        assert!(body.contains("export async function createUser("), "createUser missing");
        assert!(body.contains("export async function updateUser("), "updateUser missing");
        assert!(body.contains("export async function deleteUser("), "deleteUser missing");
    }

    #[test]
    fn imports_types_from_generated_types() {
        let r = synth_resource_all_verbs();
        let body = build_resource_api(&r);
        assert!(body.contains("from '@/generated/types/users'"), "must import from @/generated/types/users");
        assert!(body.contains("from '@/generated/types/meltdown'"), "must import MeltDownResponse");
    }

    #[test]
    fn list_fn_uses_correct_path() {
        let r = synth_resource_all_verbs();
        let body = build_resource_api(&r);
        // URL construction uses the table name
        assert!(body.contains("users/"), "list fn must reference users/ path");
    }

    #[test]
    fn delete_fn_returns_id_shape() {
        let r = synth_resource_all_verbs();
        let body = build_resource_api(&r);
        assert!(body.contains("ApiResult<{ id: number }>"), "delete must return id shape");
    }

    #[test]
    fn no_raw_fetch_in_types_import() {
        let r = synth_resource_all_verbs();
        let body = build_resource_api(&r);
        // No `: any` anywhere
        assert!(!body.contains(": any"), "no :any in api file");
        assert!(!body.contains("as any"), "no as any in api file");
    }

    #[test]
    fn plural_of_pascal_for_users() {
        assert_eq!(plural_of_pascal("User", "users"), "Users");
    }

    #[test]
    fn plural_of_pascal_for_user_accounts() {
        assert_eq!(plural_of_pascal("UserAccount", "user_accounts"), "UserAccounts");
    }
}
