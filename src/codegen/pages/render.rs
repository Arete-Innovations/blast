//! Vue SFC bodies for per-resource CRUD pages.
//!
//! Output:
//!   - `frontend/src/pages/generated/<r>/ListPage.vue`
//!   - `frontend/src/pages/generated/<r>/DetailPage.vue`
//!   - `frontend/src/pages/generated/<r>/CreatePage.vue`
//!   - `frontend/src/pages/generated/<r>/EditPage.vue`
//!
//! `CreatePage` and `EditPage` wrap the `<CreateForm>` / `<EditForm>`
//! components emitted by the `components` codegen pass.
//!
//! `ListPage` uses PrimeVue DataTable in lazy mode wired through
//! the canonical list endpoint contract: `?page&page_size&sort=-col&filter[col]=val`.

use crate::codegen::structs::naming::type_stem_for_resource;
use crate::state::{FieldVariant, ResourceState, Verb};

/// Build all CRUD page files for a resource. Returns `(filename, body)` pairs.
pub fn pages_for_resource(r: &ResourceState) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if r.verbs.contains_key(&Verb::List) {
        out.push(("ListPage.vue".to_string(), build_list_page(r)));
    }
    if r.verbs.contains_key(&Verb::Get) {
        out.push(("DetailPage.vue".to_string(), build_detail_page(r)));
    }
    if r.verbs.contains_key(&Verb::Create) {
        out.push(("CreatePage.vue".to_string(), build_create_page(r)));
    }
    if r.verbs.contains_key(&Verb::Update) {
        out.push(("EditPage.vue".to_string(), build_edit_page(r)));
    }
    out
}

/// Lazy-mode DataTable wired to the list endpoint contract.
/// Sort header maps to `sort=-col`. Filter row maps to `filter[col]=val`.
/// Page + page-size from `page` + `page_size`.
pub fn build_list_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let label = pascal_label(table);
    let plural = list_fn_plural(&stem, table);

    let public_cols: Vec<&str> = r.fields.iter().filter(|(_, f)| f.variants.contains(&FieldVariant::Public)).map(|(name, _)| name.as_str()).collect();
    let columns_html = public_cols
        .iter()
        .map(|c| format!("        <Column field=\"{c}\" header=\"{header}\" :sortable=\"true\" />", c = c, header = humanize(c)))
        .collect::<Vec<_>>()
        .join("\n");

    let has_create = r.verbs.contains_key(&Verb::Create);
    let has_get = r.verbs.contains_key(&Verb::Get);
    let has_update = r.verbs.contains_key(&Verb::Update);
    let has_delete = r.verbs.contains_key(&Verb::Delete);

    let mut delete_imports = String::new();
    if has_delete {
        delete_imports.push_str(&format!("import {{ delete{stem} }} from '@/generated/api/{table}'\n"));
    }

    let mut header_actions = String::new();
    if has_create {
        header_actions.push_str(
            "      <router-link :to=\"{ name: 'generated-{table}-create' }\" class=\"{table}-list-create-link\">\n"
                .replace("{table}", table)
                .as_str(),
        );
        header_actions.push_str("        <Button label=\"New\" icon=\"pi pi-plus\" />\n");
        header_actions.push_str("      </router-link>\n");
    }

    let mut row_actions = String::new();
    let mut has_actions_col = false;
    if has_get {
        row_actions.push_str(
            "            <router-link :to=\"{ name: 'generated-{table}-detail', params: { id: slot_props.data.id } }\" class=\"{table}-list-action\">View</router-link>\n"
                .replace("{table}", table)
                .as_str(),
        );
        has_actions_col = true;
    }
    if has_update {
        row_actions.push_str(
            "            <router-link :to=\"{ name: 'generated-{table}-edit', params: { id: slot_props.data.id } }\" class=\"{table}-list-action\">Edit</router-link>\n"
                .replace("{table}", table)
                .as_str(),
        );
        has_actions_col = true;
    }
    if has_delete {
        row_actions.push_str("            <Button label=\"Delete\" severity=\"danger\" text @click=\"on_delete(slot_props.data.id)\" />\n");
        has_actions_col = true;
    }

    let actions_column = if has_actions_col {
        format!(
            "        <Column header=\"Actions\">\n          <template #body=\"slot_props\">\n            <div class=\"{table}-list-actions\">\n{row_actions}            </div>\n          </template>\n        </Column>\n",
            table = table,
            row_actions = row_actions,
        )
    } else {
        String::new()
    };

    let delete_handler = if has_delete {
        format!(
            "async function on_delete(id: number): Promise<void> {{\n  if (!window.confirm('Delete this {table} record?')) return\n  const result = await delete{stem}(id)\n  if (result.error !== null) {{\n    error_message.value = result.error.error.message\n    return\n  }}\n  await load(last_event.value)\n}}\n",
            table = table,
            stem = stem,
        )
    } else {
        String::new()
    };

    format!(
        "<!-- AUTO-GENERATED — do not edit -->\n\
<script setup lang=\"ts\">\n\
import {{ ref }} from 'vue'\n\
import DataTable from 'primevue/datatable'\n\
import Column from 'primevue/column'\n\
import Button from 'primevue/button'\n\
import {{ list{plural} }} from '@/generated/api/{table}'\n\
{delete_imports}\
import type {{ {stem}Public }} from '@/generated/types/{table}'\n\
\n\
interface LazyEvent {{\n\
  first?: number\n\
  rows?: number\n\
  sortField?: string | null\n\
  sortOrder?: number | null\n\
  filters?: {{ [key: string]: {{ value: unknown; matchMode?: string }} }}\n\
}}\n\
\n\
const items = ref<{stem}Public[]>([])\n\
const total_records = ref<number>(0)\n\
const loading = ref<boolean>(false)\n\
const error_message = ref<string | null>(null)\n\
const last_event = ref<LazyEvent>({{ first: 0, rows: 25 }})\n\
\n\
function build_sort(event: LazyEvent): string | null {{\n\
  const field = event.sortField\n\
  if (field === null || field === undefined) return null\n\
  const order = event.sortOrder === undefined || event.sortOrder === null ? 1 : event.sortOrder\n\
  return order < 0 ? `-${{field}}` : field\n\
}}\n\
\n\
function build_filter(event: LazyEvent): {{ [key: string]: string | number | boolean | null }} | null {{\n\
  if (!event.filters) return null\n\
  const out: {{ [key: string]: string | number | boolean | null }} = {{}}\n\
  for (const [key, meta] of Object.entries(event.filters)) {{\n\
    if (meta === undefined || meta === null) continue\n\
    const value = meta.value\n\
    if (value === null || value === undefined || value === '') continue\n\
    out[key] = value as string | number | boolean | null\n\
  }}\n\
  return Object.keys(out).length > 0 ? out : null\n\
}}\n\
\n\
async function load(event: LazyEvent): Promise<void> {{\n\
  loading.value = true\n\
  error_message.value = null\n\
  last_event.value = event\n\
  const first = event.first === undefined ? 0 : event.first\n\
  const rows = event.rows === undefined ? 25 : event.rows\n\
  const page = Math.floor(first / Math.max(rows, 1)) + 1\n\
  const result = await list{plural}({{\n\
    page,\n\
    page_size: rows,\n\
    sort: build_sort(event),\n\
    filter: build_filter(event),\n\
  }})\n\
  loading.value = false\n\
  if (result.error !== null) {{\n\
    error_message.value = result.error.error.message\n\
    items.value = []\n\
    total_records.value = 0\n\
    return\n\
  }}\n\
  items.value = result.data === null ? [] : result.data\n\
  total_records.value = items.value.length\n\
}}\n\
\n\
{delete_handler}\
</script>\n\
\n\
<template>\n\
  <section class=\"{table}-list-page\">\n\
    <header class=\"{table}-list-header\">\n\
      <h1 class=\"{table}-list-title\">{label}</h1>\n\
{header_actions}    </header>\n\
    <div v-if=\"error_message !== null\" class=\"{table}-list-error\" role=\"alert\">\n\
      {{{{ error_message }}}}\n\
    </div>\n\
    <DataTable\n\
      :value=\"items\"\n\
      :loading=\"loading\"\n\
      lazy\n\
      paginator\n\
      :rows=\"25\"\n\
      :rows-per-page-options=\"[10, 25, 50, 100]\"\n\
      :total-records=\"total_records\"\n\
      :first=\"0\"\n\
      data-key=\"id\"\n\
      striped-rows\n\
      removable-sort\n\
      filter-display=\"row\"\n\
      @page=\"load\"\n\
      @sort=\"load\"\n\
      @filter=\"load\"\n\
      @load=\"load\"\n\
    >\n\
{columns_html}\n\
{actions_column}    </DataTable>\n\
  </section>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .{table}-list-page {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-lg);\n\
    padding: var(--app-space-lg);\n\
  }}\n\
  .{table}-list-header {{\n\
    display: flex;\n\
    align-items: baseline;\n\
    justify-content: space-between;\n\
    gap: var(--app-space-md);\n\
  }}\n\
  .{table}-list-title {{\n\
    margin: 0;\n\
    font-size: var(--app-text-lg);\n\
    font-weight: var(--app-font-weight-semibold);\n\
  }}\n\
  .{table}-list-error {{\n\
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));\n\
  }}\n\
  .{table}-list-actions {{\n\
    display: inline-flex;\n\
    gap: var(--app-space-sm);\n\
    align-items: center;\n\
  }}\n\
  .{table}-list-action {{\n\
    color: var(--p-primary-color);\n\
    text-decoration: underline;\n\
  }}\n\
}}\n\
</style>\n",
        table = table,
        stem = stem,
        plural = plural,
        label = label,
        columns_html = columns_html,
        actions_column = actions_column,
        delete_imports = delete_imports,
        delete_handler = delete_handler,
        header_actions = header_actions,
    )
}

/// Read-only field display, fetched via `get<Stem>` and routed off `:id`.
pub fn build_detail_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let label = pascal_label(table);

    let public_cols: Vec<(&str, &str)> = r
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Public))
        .map(|(name, _)| (name.as_str(), name.as_str()))
        .collect();
    let rows_html = public_cols
        .iter()
        .map(|(field, _)| {
            format!(
                "      <div class=\"{table}-detail-row\">\n        <dt class=\"{table}-detail-label\">{label}</dt>\n        <dd class=\"{table}-detail-value\">{{{{ format_value(item['{field}']) }}}}</dd>\n      </div>",
                table = table,
                label = humanize(field),
                field = field
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let has_update = r.verbs.contains_key(&Verb::Update);
    let edit_link = if has_update {
        format!(
            "      <router-link :to=\"{{ name: 'generated-{table}-edit', params: {{ id: props.id }} }}\" class=\"{table}-detail-edit\">\n        <Button label=\"Edit\" icon=\"pi pi-pencil\" />\n      </router-link>\n",
            table = table,
        )
    } else {
        String::new()
    };

    format!(
        "<!-- AUTO-GENERATED — do not edit -->\n\
<script setup lang=\"ts\">\n\
import {{ onMounted, ref, watch }} from 'vue'\n\
import Button from 'primevue/button'\n\
import {{ get{stem} }} from '@/generated/api/{table}'\n\
import type {{ {stem}Public }} from '@/generated/types/{table}'\n\
\n\
const props = defineProps<{{ id: number }}>()\n\
\n\
const item = ref<{stem}Public | null>(null)\n\
const loading = ref<boolean>(false)\n\
const error_message = ref<string | null>(null)\n\
\n\
function format_value(value: unknown): string {{\n\
  if (value === null || value === undefined) return '—'\n\
  if (typeof value === 'object') return JSON.stringify(value)\n\
  return String(value)\n\
}}\n\
\n\
async function load(id: number): Promise<void> {{\n\
  loading.value = true\n\
  error_message.value = null\n\
  const result = await get{stem}(id)\n\
  loading.value = false\n\
  if (result.error !== null) {{\n\
    error_message.value = result.error.error.message\n\
    return\n\
  }}\n\
  item.value = result.data\n\
}}\n\
\n\
onMounted(() => {{ void load(props.id) }})\n\
watch(() => props.id, (next) => {{ void load(next) }})\n\
</script>\n\
\n\
<template>\n\
  <section class=\"{table}-detail-page\">\n\
    <header class=\"{table}-detail-header\">\n\
      <h1 class=\"{table}-detail-title\">{label}</h1>\n\
{edit_link}    </header>\n\
    <div v-if=\"error_message !== null\" class=\"{table}-detail-error\" role=\"alert\">\n\
      {{{{ error_message }}}}\n\
    </div>\n\
    <div v-if=\"loading\" class=\"{table}-detail-loading\">Loading…</div>\n\
    <dl v-else-if=\"item !== null\" class=\"{table}-detail-grid\">\n\
{rows_html}\n\
    </dl>\n\
  </section>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .{table}-detail-page {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-lg);\n\
    padding: var(--app-space-lg);\n\
  }}\n\
  .{table}-detail-header {{\n\
    display: flex;\n\
    align-items: baseline;\n\
    justify-content: space-between;\n\
  }}\n\
  .{table}-detail-title {{\n\
    margin: 0;\n\
    font-size: var(--app-text-lg);\n\
  }}\n\
  .{table}-detail-grid {{\n\
    display: grid;\n\
    grid-template-columns: minmax(8rem, max-content) 1fr;\n\
    gap: var(--app-space-sm) var(--app-space-md);\n\
    margin: 0;\n\
  }}\n\
  .{table}-detail-row {{\n\
    display: contents;\n\
  }}\n\
  .{table}-detail-label {{\n\
    font-weight: var(--app-font-weight-semibold);\n\
    color: var(--p-text-muted-color);\n\
  }}\n\
  .{table}-detail-value {{\n\
    margin: 0;\n\
  }}\n\
  .{table}-detail-error {{\n\
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));\n\
  }}\n\
}}\n\
</style>\n",
        table = table,
        stem = stem,
        label = label,
        rows_html = rows_html,
        edit_link = edit_link,
    )
}

/// `CreatePage.vue` wraps `<CreateForm>` and navigates back on success.
pub fn build_create_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let label = pascal_label(table);
    let has_list = r.verbs.contains_key(&Verb::List);

    let nav_after = if has_list {
        format!("  await router.push({{ name: 'generated-{table}-list' }})\n", table = table)
    } else {
        "  router.back()\n".to_string()
    };

    format!(
        "<!-- AUTO-GENERATED — do not edit -->\n\
<script setup lang=\"ts\">\n\
import {{ useRouter }} from 'vue-router'\n\
import CreateForm from '@/components/generated/forms/{table}/CreateForm.vue'\n\
\n\
const router = useRouter()\n\
\n\
async function on_created(): Promise<void> {{\n\
{nav_after}}}\n\
\n\
function on_cancel(): void {{\n\
  router.back()\n\
}}\n\
</script>\n\
\n\
<template>\n\
  <section class=\"{table}-create-page\">\n\
    <header class=\"{table}-create-header\">\n\
      <h1 class=\"{table}-create-title\">New {label_singular}</h1>\n\
    </header>\n\
    <CreateForm @created=\"on_created\" @cancel=\"on_cancel\" />\n\
  </section>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .{table}-create-page {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-lg);\n\
    padding: var(--app-space-lg);\n\
    max-width: 48rem;\n\
  }}\n\
  .{table}-create-title {{\n\
    margin: 0;\n\
    font-size: var(--app-text-lg);\n\
  }}\n\
}}\n\
</style>\n",
        table = table,
        label_singular = label_singular(&label),
        nav_after = nav_after,
    )
}

/// `EditPage.vue` fetches the entity, then wraps `<EditForm>`.
pub fn build_edit_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let label = pascal_label(table);
    let has_get = r.verbs.contains_key(&Verb::Get);

    let nav_after = if has_get {
        format!("  await router.push({{ name: 'generated-{table}-detail', params: {{ id: props.id }} }})\n", table = table)
    } else {
        "  router.back()\n".to_string()
    };

    format!(
        "<!-- AUTO-GENERATED — do not edit -->\n\
<script setup lang=\"ts\">\n\
import {{ onMounted, ref, watch }} from 'vue'\n\
import {{ useRouter }} from 'vue-router'\n\
import EditForm from '@/components/generated/forms/{table}/EditForm.vue'\n\
import {{ get{stem} }} from '@/generated/api/{table}'\n\
import type {{ {stem}Public }} from '@/generated/types/{table}'\n\
\n\
const props = defineProps<{{ id: number }}>()\n\
const router = useRouter()\n\
\n\
const entity = ref<{stem}Public | null>(null)\n\
const loading = ref<boolean>(false)\n\
const error_message = ref<string | null>(null)\n\
\n\
async function load(): Promise<void> {{\n\
  loading.value = true\n\
  error_message.value = null\n\
  const result = await get{stem}(props.id)\n\
  loading.value = false\n\
  if (result.error !== null) {{\n\
    error_message.value = result.error.error.message\n\
    return\n\
  }}\n\
  entity.value = result.data\n\
}}\n\
\n\
async function on_updated(): Promise<void> {{\n\
{nav_after}}}\n\
\n\
function on_cancel(): void {{\n\
  router.back()\n\
}}\n\
\n\
onMounted(load)\n\
watch(() => props.id, load)\n\
</script>\n\
\n\
<template>\n\
  <section class=\"{table}-edit-page\">\n\
    <header class=\"{table}-edit-header\">\n\
      <h1 class=\"{table}-edit-title\">Edit {label_singular} #{{{{ id }}}}</h1>\n\
    </header>\n\
    <div v-if=\"error_message !== null\" class=\"{table}-edit-error\" role=\"alert\">\n\
      {{{{ error_message }}}}\n\
    </div>\n\
    <div v-if=\"loading\" class=\"{table}-edit-loading\">Loading…</div>\n\
    <EditForm v-else-if=\"entity !== null\" :entity=\"entity\" @updated=\"on_updated\" @cancel=\"on_cancel\" />\n\
  </section>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .{table}-edit-page {{\n\
    display: flex;\n\
    flex-direction: column;\n\
    gap: var(--app-space-lg);\n\
    padding: var(--app-space-lg);\n\
    max-width: 48rem;\n\
  }}\n\
  .{table}-edit-title {{\n\
    margin: 0;\n\
    font-size: var(--app-text-lg);\n\
  }}\n\
  .{table}-edit-error {{\n\
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));\n\
  }}\n\
}}\n\
</style>\n",
        table = table,
        stem = stem,
        label_singular = label_singular(&label),
        nav_after = nav_after,
    )
}

/// Plural form of the type stem to match `frontend_api::list<Plural>`.
/// E.g. `User`/`users` -> `Users`; `UserAccount`/`user_accounts` -> `UserAccounts`.
fn list_fn_plural(singular: &str, table: &str) -> String {
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
    if out.starts_with(singular) {
        out
    } else {
        format!("{singular}s")
    }
}

fn label_singular(plural_label: &str) -> String {
    crate::codegen::structs::naming::singularize(plural_label)
        .split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(), // allow: empty word segment is not a failure
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn pascal_label(table: &str) -> String {
    table
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(), // allow: empty word segment is not a failure
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Underscored snake_case to a Title Case label.
fn humanize(snake: &str) -> String {
    snake
        .split('_')
        .filter(|w| !w.is_empty())
        .enumerate()
        .map(|(i, w)| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(), // allow: empty word segment is not a failure
                Some(first) => {
                    if i == 0 {
                        let upper: String = first.to_uppercase().collect();
                        upper + chars.as_str()
                    } else {
                        let lower: String = first.to_lowercase().collect();
                        lower + chars.as_str()
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::names::{FieldName, ResourceName};
    use crate::state::resource::{AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState, RESOURCE_SCHEMA_VERSION};
    use crate::state::SqlType;
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_resource_full_crud() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let public_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Public].into_iter().collect();
        let all_v: BTreeSet<FieldVariant> = [FieldVariant::Db, FieldVariant::Insertable, FieldVariant::Patch, FieldVariant::Public].into_iter().collect();

        fields.insert(
            FieldName::new("id"),
            FieldState {
                sql_type: SqlType::new("Int8"),
                variants: public_v.clone(),
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
        verbs.insert(
            Verb::List,
            VerbState {
                auth: AuthMode::Public,
                list_options: Some(ListOptions {
                    paginated: true,
                    filterable_columns: BTreeMap::new(),
                    sortable_columns: BTreeSet::new(),
                    default_sort: None,
                    max_page_size: None,
                }),
            },
        );
        for v in [Verb::Get, Verb::Create, Verb::Update, Verb::Delete] {
            verbs.insert(
                v,
                VerbState {
                    auth: AuthMode::Public,
                    list_options: None,
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
        }
    }

    #[test]
    fn list_page_uses_lazy_mode_and_list_api() {
        let r = synth_resource_full_crud();
        let body = build_list_page(&r);
        assert!(body.contains("lazy"), "DataTable must be lazy");
        assert!(body.contains("listUsers"), "must call listUsers API");
        assert!(body.contains("page,"), "must pass page");
        assert!(body.contains("page_size: rows"), "must pass page_size");
        assert!(body.contains("`-${field}`"), "sort must format with leading dash");
    }

    #[test]
    fn list_page_includes_create_link_when_create_verb_present() {
        let r = synth_resource_full_crud();
        let body = build_list_page(&r);
        assert!(body.contains("generated-users-create"), "create link required");
    }

    #[test]
    fn list_page_emits_columns_for_public_fields() {
        let r = synth_resource_full_crud();
        let body = build_list_page(&r);
        assert!(body.contains("field=\"id\""));
        assert!(body.contains("field=\"email\""));
    }

    #[test]
    fn detail_page_renders_field_grid() {
        let r = synth_resource_full_crud();
        let body = build_detail_page(&r);
        assert!(body.contains("getUser"));
        assert!(body.contains("UserPublic"));
        assert!(body.contains("format_value(item['email'])"));
    }

    #[test]
    fn create_page_wraps_create_form_component() {
        let r = synth_resource_full_crud();
        let body = build_create_page(&r);
        assert!(body.contains("import CreateForm from '@/components/generated/forms/users/CreateForm.vue'"));
        assert!(body.contains("<CreateForm"));
    }

    #[test]
    fn edit_page_wraps_edit_form_with_entity_prop() {
        let r = synth_resource_full_crud();
        let body = build_edit_page(&r);
        assert!(body.contains("import EditForm from '@/components/generated/forms/users/EditForm.vue'"));
        assert!(body.contains(":entity=\"entity\""));
        assert!(body.contains("getUser"));
    }

    #[test]
    fn pages_for_resource_emits_only_when_verb_present() {
        let mut r = synth_resource_full_crud();
        r.verbs.shift_remove(&Verb::Create);
        r.verbs.shift_remove(&Verb::Delete);
        let pairs = pages_for_resource(&r);
        let names: Vec<&str> = pairs.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"ListPage.vue"));
        assert!(names.contains(&"DetailPage.vue"));
        assert!(names.contains(&"EditPage.vue"));
        assert!(!names.contains(&"CreatePage.vue"));
    }

    #[test]
    fn list_fn_plural_handles_users() {
        assert_eq!(list_fn_plural("User", "users"), "Users");
        assert_eq!(list_fn_plural("UserAccount", "user_accounts"), "UserAccounts");
    }
}
