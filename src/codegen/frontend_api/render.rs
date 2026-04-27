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

use crate::codegen::structs::naming::type_stem_for_resource;
use crate::state::{ResourceState, Verb};

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
        out.push_str(&format!(
            "import {{ {names} }} from '@/generated/types/{table}'\n",
            names = seen.join(", "),
            table = table,
        ));
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
/// so composables can store `ref<UserPublic[] | null>` without needing
/// to know about the pagination envelope.
fn render_list_fn(table: &str, singular: &str, plural: &str) -> String {
    format!(
        "interface {singular}ListEnvelope {{\n\
  items: {singular}Public[]\n\
  total: number\n\
  page: number\n\
  page_size: number\n\
}}\n\
\n\
export async function list{plural}(\n\
  params: {{ page?: number; page_size?: number; sort?: string | null; filter?: {{ [key: string]: string | number | boolean | null | undefined }} | null }},\n\
  signal?: AbortSignal,\n\
): ApiResult<{singular}Public[]> {{\n\
  const url = new URL(`/api/{table}/`, window.location.origin)\n\
  if (params.page !== undefined) {{\n\
    url.searchParams.set('page', String(params.page))\n\
  }}\n\
  if (params.page_size !== undefined) {{\n\
    url.searchParams.set('page_size', String(params.page_size))\n\
  }}\n\
  if (params.sort !== undefined && params.sort !== null) {{\n\
    url.searchParams.set('sort', params.sort)\n\
  }}\n\
  if (params.filter !== null && params.filter !== undefined) {{\n\
    for (const [key, val] of Object.entries(params.filter)) {{\n\
      if (val === null || val === undefined) {{\n\
        continue\n\
      }}\n\
      const serialized = typeof val === 'object' ? JSON.stringify(val) : String(val)\n\
      url.searchParams.set(`filter[${{key}}]`, serialized)\n\
    }}\n\
  }}\n\
  const path = url.pathname + url.search\n\
  try {{\n\
    const res = await fetch(path, {{\n\
      headers: {{ ...auth_header(), Accept: 'application/json' }},\n\
      signal,\n\
    }})\n\
    if (!res.ok) {{\n\
      const err = (await res.json()) as MeltDownResponse\n\
      return {{ data: null, error: err }}\n\
    }}\n\
    const envelope = (await res.json()) as {singular}ListEnvelope\n\
    return {{ data: envelope.items, error: null }}\n\
  }} catch (e) {{\n\
    if (e instanceof DOMException && e.name === 'AbortError') {{\n\
      return {{ data: null, error: null }}\n\
    }}\n\
    const err: MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network error', context: null }} }}\n\
    return {{ data: null, error: err }}\n\
  }}\n\
}}\n",
        table = table,
        singular = singular,
        plural = plural,
    )
}

/// `getUser` — matches `get{} as apiGet` alias.
fn render_get_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function get{singular}(\n\
  id: number,\n\
  signal?: AbortSignal,\n\
): ApiResult<{singular}Public> {{\n\
  try {{\n\
    const res = await fetch(`/api/{table}/${{id}}`, {{\n\
      headers: {{ ...auth_header(), Accept: 'application/json' }},\n\
      signal,\n\
    }})\n\
    if (!res.ok) {{\n\
      const err = (await res.json()) as MeltDownResponse\n\
      return {{ data: null, error: err }}\n\
    }}\n\
    const data = (await res.json()) as {singular}Public\n\
    return {{ data, error: null }}\n\
  }} catch (e) {{\n\
    if (e instanceof DOMException && e.name === 'AbortError') {{\n\
      return {{ data: null, error: null }}\n\
    }}\n\
    const err: MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network error', context: null }} }}\n\
    return {{ data: null, error: err }}\n\
  }}\n\
}}\n",
        table = table,
        singular = singular,
    )
}

/// `createUser` — matches `create{} as apiCreate` alias.
fn render_create_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function create{singular}(\n\
  body: {singular}Insertable,\n\
): ApiResult<{singular}Public> {{\n\
  try {{\n\
    const res = await fetch(`/api/{table}/`, {{\n\
      method: 'POST',\n\
      headers: {{ ...auth_header(), 'Content-Type': 'application/json', Accept: 'application/json' }},\n\
      body: JSON.stringify(body),\n\
    }})\n\
    if (!res.ok) {{\n\
      const err = (await res.json()) as MeltDownResponse\n\
      return {{ data: null, error: err }}\n\
    }}\n\
    const data = (await res.json()) as {singular}Public\n\
    return {{ data, error: null }}\n\
  }} catch (e) {{\n\
    const err: MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network error', context: null }} }}\n\
    return {{ data: null, error: err }}\n\
  }}\n\
}}\n",
        table = table,
        singular = singular,
    )
}

/// `updateUser` — matches `update{} as apiUpdate` alias.
fn render_update_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function update{singular}(\n\
  id: number,\n\
  patch: {singular}Patch,\n\
): ApiResult<{singular}Public> {{\n\
  try {{\n\
    const res = await fetch(`/api/{table}/${{id}}`, {{\n\
      method: 'PATCH',\n\
      headers: {{ ...auth_header(), 'Content-Type': 'application/json', Accept: 'application/json' }},\n\
      body: JSON.stringify(patch),\n\
    }})\n\
    if (!res.ok) {{\n\
      const err = (await res.json()) as MeltDownResponse\n\
      return {{ data: null, error: err }}\n\
    }}\n\
    const data = (await res.json()) as {singular}Public\n\
    return {{ data, error: null }}\n\
  }} catch (e) {{\n\
    const err: MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network error', context: null }} }}\n\
    return {{ data: null, error: err }}\n\
  }}\n\
}}\n",
        table = table,
        singular = singular,
    )
}

/// `deleteUser` — matches `delete{} as apiDelete` alias.
fn render_delete_fn(table: &str, singular: &str) -> String {
    format!(
        "export async function delete{singular}(\n\
  id: number,\n\
): ApiResult<{{ id: number }}> {{\n\
  try {{\n\
    const res = await fetch(`/api/{table}/${{id}}`, {{\n\
      method: 'DELETE',\n\
      headers: {{ ...auth_header(), Accept: 'application/json' }},\n\
    }})\n\
    if (!res.ok) {{\n\
      const err = (await res.json()) as MeltDownResponse\n\
      return {{ data: null, error: err }}\n\
    }}\n\
    return {{ data: {{ id }}, error: null }}\n\
  }} catch (e) {{\n\
    const err: MeltDownResponse = {{ error: {{ code: 0, type: 'NetworkError', message: 'Network error', context: null }} }}\n\
    return {{ data: null, error: err }}\n\
  }}\n\
}}\n",
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
    use super::*;
    use crate::state::names::{FieldName, ResourceName};
    use crate::state::resource::{
        AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState,
        RESOURCE_SCHEMA_VERSION,
    };
    use crate::state::SqlType;
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_resource_all_verbs() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let all_v: BTreeSet<FieldVariant> = [
            FieldVariant::Db,
            FieldVariant::Insertable,
            FieldVariant::Patch,
            FieldVariant::Public,
        ]
        .into_iter()
        .collect();
        let id_v: BTreeSet<FieldVariant> =
            [FieldVariant::Db, FieldVariant::Public].into_iter().collect();

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
            verbs.insert(v, VerbState { auth: AuthMode::Public, list_options: list_opts });
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
        assert!(
            body.contains("from '@/generated/types/users'"),
            "must import from @/generated/types/users"
        );
        assert!(
            body.contains("from '@/generated/types/meltdown'"),
            "must import MeltDownResponse"
        );
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
        assert!(
            body.contains("ApiResult<{ id: number }>"),
            "delete must return id shape"
        );
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
