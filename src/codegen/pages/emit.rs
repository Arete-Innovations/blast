//! Per-verb Vue page SFC builders.
//!
//! Each function returns the body of one page component (excluding the
//! hash-marker header, which the runner prepends). All emitted files live at
//! `frontend/src/pages/<resource>/`.
//!
//! Layout selection per SPEC_FRONTEND_ROUTING:
//!   List   → table
//!   Get    → cards
//!   Create → cards
//!   Update → cards
//!
//! Delete is not a page; the action lives in the detail/list pages.

use crate::codegen::vue::naming::{pascal_case, singularize};
use crate::state::ResourceState;

fn type_stem(r: &ResourceState) -> String {
    let singular = match r.singular_override.as_deref() {
        Some(s) => s.to_string(),
        None => singularize(r.name.as_str()), // allow: singular_override is a naming hint, absence is not a failure
    };
    pascal_case(&singular)
}

fn resource_table(r: &ResourceState) -> &str {
    r.name.as_str()
}

// ---------------------------------------------------------------------------
// ListPage
// ---------------------------------------------------------------------------

/// `<Resource>ListPage.vue` — layout: table.
pub fn build_list_page(r: &ResourceState) -> String {
    let table = resource_table(r);
    let pascal = type_stem(r);
    let composable = format!("use{}sList", pascal);
    let list_component = format!("{}List", pascal);
    let route_create = format!("{}.create", table);
    let route_detail = format!("{}.detail", table);
    let route_edit = format!("{}.edit", table);
    let label = pascal_label(table);
    let plural_label = &label;

    format!(
        r#"<script setup lang="ts">
import {{ useRouter }} from 'vue-router'
import Button from 'primevue/button'
import {{ IC }} from '@/icons'
import {{ {composable} }} from '@/generated/composables/{table}'
import {{ {list_component} }} from '@/generated/components/{table}'
import {{ ROUTE_NAMES }} from '@/generated/router/route-names'

const router = useRouter()
const list_state = {composable}()

function on_create(): void {{
  router.push({{ name: ROUTE_NAMES['{route_create}'] }})
}}

function on_edit(id: number): void {{
  router.push({{ name: ROUTE_NAMES['{route_edit}'], params: {{ id }} }})
}}

function on_view(id: number): void {{
  router.push({{ name: ROUTE_NAMES['{route_detail}'], params: {{ id }} }})
}}
</script>

<template>
  <PageShell layout="table">
    <template #header>
      <span class="page-title">{plural_label}</span>
      <Button label="New {label}" :icon="IC.add" @click="on_create" />
    </template>
    <{list_component}
      :data="list_state.data"
      :page="list_state.page"
      :sort="list_state.sort"
      :filter="list_state.filter"
      @view="on_view"
      @edit="on_edit"
    />
  </PageShell>
</template>
"#,
        composable = composable,
        table = table,
        list_component = list_component,
        route_create = route_create,
        route_detail = route_detail,
        route_edit = route_edit,
        label = label,
        plural_label = plural_label,
    )
}

// ---------------------------------------------------------------------------
// DetailPage
// ---------------------------------------------------------------------------

/// `<Resource>DetailPage.vue` — layout: cards.
pub fn build_detail_page(r: &ResourceState) -> String {
    let table = resource_table(r);
    let pascal = type_stem(r);
    let composable = format!("use{}", pascal);
    let route_edit = format!("{}.edit", table);
    let route_list = format!("{}.list", table);
    let label = pascal_label(table);
    let public_type = format!("{}Public", pascal);

    format!(
        r#"<script setup lang="ts">
import {{ computed }} from 'vue'
import {{ useRouter }} from 'vue-router'
import Button from 'primevue/button'
import {{ IC }} from '@/icons'
import type {{ {public_type} }} from '@/generated/types/{table}'
import {{ {composable} }} from '@/generated/composables/{table}'
import {{ ROUTE_NAMES }} from '@/generated/router/route-names'

const props = defineProps<{{ id: number }}>()
const router = useRouter()
const resource_id = computed<number>(() => props.id)
const {{ data }} = {composable}(resource_id)

function on_back(): void {{
  router.push({{ name: ROUTE_NAMES['{route_list}'] }})
}}

function on_edit(): void {{
  router.push({{ name: ROUTE_NAMES['{route_edit}'], params: {{ id: props.id }} }})
}}
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <span class="page-title">{label}</span>
      <span class="page-header-actions">
        <Button label="Back" :icon="IC.back" severity="secondary" @click="on_back" />
        <Button label="Edit" :icon="IC.edit" @click="on_edit" />
      </span>
    </template>
    <div v-if="data" class="{table}-detail-card">
      <pre class="{table}-detail-json">{{ {{ data }} }}</pre>
    </div>
  </PageShell>
</template>
"#,
        public_type = public_type,
        table = table,
        composable = composable,
        route_edit = route_edit,
        route_list = route_list,
        label = label,
    )
}

// ---------------------------------------------------------------------------
// CreatePage
// ---------------------------------------------------------------------------

/// `<Resource>CreatePage.vue` — layout: cards.
pub fn build_create_page(r: &ResourceState) -> String {
    let table = resource_table(r);
    let pascal = type_stem(r);
    let create_composable = format!("useCreate{}", pascal);
    let form_component = format!("{}Form", pascal);
    let route_list = format!("{}.list", table);
    let route_detail = format!("{}.detail", table);
    let label = pascal_label(table);
    let insertable_type = format!("{}Insertable", pascal);

    format!(
        r#"<script setup lang="ts">
import {{ ref }} from 'vue'
import {{ useRouter }} from 'vue-router'
import Button from 'primevue/button'
import {{ IC }} from '@/icons'
import type {{ {insertable_type} }} from '@/generated/types/{table}'
import {{ {create_composable} }} from '@/generated/composables/{table}'
import {{ {form_component} }} from '@/generated/components/{table}'
import {{ ROUTE_NAMES }} from '@/generated/router/route-names'

const router = useRouter()
const submitting = ref<boolean>(false)
const submit_error = ref<string | undefined>(undefined)
const {{ execute }} = {create_composable}()

function on_back(): void {{
  router.push({{ name: ROUTE_NAMES['{route_list}'] }})
}}

async function on_submit(payload: {insertable_type}): Promise<void> {{
  submitting.value = true
  submit_error.value = undefined
  const {{ data, error }} = await execute(payload)
  submitting.value = false
  if (error !== null) {{
    submit_error.value = error.message
    return
  }}
  if (data !== null) {{
    router.push({{ name: ROUTE_NAMES['{route_detail}'], params: {{ id: data.id }} }})
  }}
}}
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <span class="page-title">New {label}</span>
      <Button label="Back" :icon="IC.back" severity="secondary" @click="on_back" />
    </template>
    <{form_component}
      mode="create"
      :submitting="submitting"
      :submit-error="submit_error"
      @submit="on_submit"
    />
  </PageShell>
</template>
"#,
        insertable_type = insertable_type,
        table = table,
        create_composable = create_composable,
        form_component = form_component,
        route_list = route_list,
        route_detail = route_detail,
        label = label,
    )
}

// ---------------------------------------------------------------------------
// EditPage
// ---------------------------------------------------------------------------

/// `<Resource>EditPage.vue` — layout: cards.
pub fn build_edit_page(r: &ResourceState) -> String {
    let table = resource_table(r);
    let pascal = type_stem(r);
    let get_composable = format!("use{}", pascal);
    let update_composable = format!("useUpdate{}", pascal);
    let form_component = format!("{}Form", pascal);
    let route_detail = format!("{}.detail", table);
    let label = pascal_label(table);
    let patch_type = format!("{}Patch", pascal);

    format!(
        r#"<script setup lang="ts">
import {{ computed, ref }} from 'vue'
import {{ useRouter }} from 'vue-router'
import Button from 'primevue/button'
import {{ IC }} from '@/icons'
import type {{ {patch_type} }} from '@/generated/types/{table}'
import {{ {get_composable}, {update_composable} }} from '@/generated/composables/{table}'
import {{ {form_component} }} from '@/generated/components/{table}'
import {{ ROUTE_NAMES }} from '@/generated/router/route-names'

const props = defineProps<{{ id: number }}>()
const router = useRouter()
const resource_id = computed<number>(() => props.id)
const submitting = ref<boolean>(false)
const submit_error = ref<string | undefined>(undefined)
const {{ data }} = {get_composable}(resource_id)
const {{ execute }} = {update_composable}()

function on_back(): void {{
  router.push({{ name: ROUTE_NAMES['{route_detail}'], params: {{ id: props.id }} }})
}}

async function on_submit(payload: {patch_type}): Promise<void> {{
  submitting.value = true
  submit_error.value = undefined
  const {{ error }} = await execute(props.id, payload)
  submitting.value = false
  if (error !== null) {{
    submit_error.value = error.message
    return
  }}
  router.push({{ name: ROUTE_NAMES['{route_detail}'], params: {{ id: props.id }} }})
}}
</script>

<template>
  <PageShell layout="cards">
    <template #header>
      <span class="page-title">Edit {label}</span>
      <Button label="Back" :icon="IC.back" severity="secondary" @click="on_back" />
    </template>
    <{form_component}
      v-if="data"
      mode="edit"
      :submitting="submitting"
      :submit-error="submit_error"
      @submit="on_submit"
    />
  </PageShell>
</template>
"#,
        patch_type = patch_type,
        table = table,
        get_composable = get_composable,
        update_composable = update_composable,
        form_component = form_component,
        route_detail = route_detail,
        label = label,
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert `snake_case_table_name` → `"Snake Case Table Name"` for display.
fn pascal_label(table: &str) -> String {
    table
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(), // allow: empty word segment is not a failure; split('_') produces it on edge input
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
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
    use crate::state::resource::{
        AuthMode, FieldState, FieldVariant, ListOptions, ResourceState, Verb, VerbState,
        RESOURCE_SCHEMA_VERSION,
    };
    use crate::state::SqlType;
    use indexmap::IndexMap;
    use std::collections::{BTreeMap, BTreeSet};

    fn synth_resource() -> ResourceState {
        let mut fields: IndexMap<FieldName, FieldState> = IndexMap::new();
        let mut all_v = BTreeSet::new();
        all_v.insert(FieldVariant::Db);
        all_v.insert(FieldVariant::Insertable);
        all_v.insert(FieldVariant::Patch);
        all_v.insert(FieldVariant::Public);

        let mut id_v = BTreeSet::new();
        id_v.insert(FieldVariant::Db);
        id_v.insert(FieldVariant::Public);

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
            FieldName::new("name"),
            FieldState {
                sql_type: SqlType::new("Varchar"),
                variants: all_v.clone(),
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
        verbs.insert(
            Verb::Get,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Create,
            VerbState {
                auth: AuthMode::Public,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Update,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );
        verbs.insert(
            Verb::Delete,
            VerbState {
                auth: AuthMode::AuthRequired,
                list_options: None,
            },
        );

        ResourceState {
            schema_version: RESOURCE_SCHEMA_VERSION,
            name: ResourceName::new("widgets"),
            fields,
            verbs,
            ws_events: None,
            singular_override: None,
            soft_delete: None,
            relations: BTreeMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // PageShell root + layout
    // -----------------------------------------------------------------------

    #[test]
    fn list_page_wraps_in_page_shell_table() {
        let r = synth_resource();
        let body = build_list_page(&r);
        assert!(body.contains("<PageShell layout=\"table\">"), "list must use table layout");
        assert!(body.contains("</PageShell>"), "must close PageShell");
    }

    #[test]
    fn detail_page_wraps_in_page_shell_cards() {
        let r = synth_resource();
        let body = build_detail_page(&r);
        assert!(body.contains("<PageShell layout=\"cards\">"), "detail must use cards layout");
    }

    #[test]
    fn create_page_wraps_in_page_shell_cards() {
        let r = synth_resource();
        let body = build_create_page(&r);
        assert!(body.contains("<PageShell layout=\"cards\">"), "create must use cards layout");
    }

    #[test]
    fn edit_page_wraps_in_page_shell_cards() {
        let r = synth_resource();
        let body = build_edit_page(&r);
        assert!(body.contains("<PageShell layout=\"cards\">"), "edit must use cards layout");
    }

    // -----------------------------------------------------------------------
    // Header slot present
    // -----------------------------------------------------------------------

    #[test]
    fn all_pages_have_header_slot() {
        let r = synth_resource();
        for (name, body) in [
            ("list", build_list_page(&r)),
            ("detail", build_detail_page(&r)),
            ("create", build_create_page(&r)),
            ("edit", build_edit_page(&r)),
        ] {
            assert!(
                body.contains("<template #header>"),
                "{} page must have #header slot",
                name
            );
        }
    }

    // -----------------------------------------------------------------------
    // Composable references
    // -----------------------------------------------------------------------

    #[test]
    fn list_page_references_list_composable() {
        let r = synth_resource();
        let body = build_list_page(&r);
        assert!(body.contains("useWidgetsList"), "list page must import useWidgetsList");
    }

    #[test]
    fn detail_page_references_get_composable() {
        let r = synth_resource();
        let body = build_detail_page(&r);
        assert!(body.contains("useWidget"), "detail page must import useWidget");
    }

    #[test]
    fn create_page_references_create_composable() {
        let r = synth_resource();
        let body = build_create_page(&r);
        assert!(body.contains("useCreateWidget"), "create page must import useCreateWidget");
    }

    #[test]
    fn edit_page_references_get_and_update_composables() {
        let r = synth_resource();
        let body = build_edit_page(&r);
        assert!(body.contains("useWidget"), "edit page must import useWidget");
        assert!(body.contains("useUpdateWidget"), "edit page must import useUpdateWidget");
    }

    // -----------------------------------------------------------------------
    // Component consumption (no duplication of Form/List logic)
    // -----------------------------------------------------------------------

    #[test]
    fn list_page_references_list_component() {
        let r = synth_resource();
        let body = build_list_page(&r);
        assert!(body.contains("WidgetList"), "list page must consume WidgetList component");
    }

    #[test]
    fn create_page_references_form_component() {
        let r = synth_resource();
        let body = build_create_page(&r);
        assert!(body.contains("WidgetForm"), "create page must consume WidgetForm component");
        assert!(body.contains("mode=\"create\""), "create page must pass mode=create");
    }

    #[test]
    fn edit_page_references_form_component() {
        let r = synth_resource();
        let body = build_edit_page(&r);
        assert!(body.contains("WidgetForm"), "edit page must consume WidgetForm component");
        assert!(body.contains("mode=\"edit\""), "edit page must pass mode=edit");
    }

    // -----------------------------------------------------------------------
    // script setup + typed props
    // -----------------------------------------------------------------------

    #[test]
    fn pages_use_script_setup_lang_ts() {
        let r = synth_resource();
        for (name, body) in [
            ("list", build_list_page(&r)),
            ("detail", build_detail_page(&r)),
            ("create", build_create_page(&r)),
            ("edit", build_edit_page(&r)),
        ] {
            assert!(
                body.contains("<script setup lang=\"ts\">"),
                "{} must use <script setup lang=\"ts\">",
                name
            );
        }
    }

    #[test]
    fn detail_and_edit_pages_declare_id_prop() {
        let r = synth_resource();
        for (name, body) in [
            ("detail", build_detail_page(&r)),
            ("edit", build_edit_page(&r)),
        ] {
            assert!(
                body.contains("defineProps<{ id: number }>()"),
                "{} page must declare typed id prop",
                name
            );
        }
    }

    // -----------------------------------------------------------------------
    // Governor compliance: no hex, no px, no inline styles, no :any
    // -----------------------------------------------------------------------

    fn assert_governor_clean(name: &str, body: &str) {
        // No hex color literals outside comment lines
        for line in body.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("<!--") || trimmed.starts_with('*') {
                continue;
            }
            for (idx, _) in line.match_indices('#') {
                let after = &line[idx + 1..];
                if after.chars().next().map(|c| c.is_ascii_hexdigit()).unwrap_or(false) {
                    panic!("{} page contains hex color: {}", name, line);
                }
            }
        }
        // No inline style attribute
        assert!(
            !body.contains(" style=\""),
            "{} page must not contain inline style attribute",
            name
        );
        assert!(
            !body.contains(":style=\""),
            "{} page must not contain :style binding",
            name
        );
        // No :any
        assert!(
            !body.contains(": any"),
            "{} page must not use `: any`",
            name
        );
        assert!(
            !body.contains("as any"),
            "{} page must not use `as any`",
            name
        );
        // No console.log
        assert!(
            !body.contains("console.log"),
            "{} page must not contain console.log",
            name
        );
        // No || {} or ?? [] silent fallbacks
        assert!(
            !body.contains("|| {}"),
            "{} page must not use `|| {{}}` fallback",
            name
        );
        assert!(
            !body.contains("?? []"),
            "{} page must not use `?? []` fallback",
            name
        );
    }

    #[test]
    fn all_pages_pass_governor_checks() {
        let r = synth_resource();
        assert_governor_clean("list", &build_list_page(&r));
        assert_governor_clean("detail", &build_detail_page(&r));
        assert_governor_clean("create", &build_create_page(&r));
        assert_governor_clean("edit", &build_edit_page(&r));
    }

    // -----------------------------------------------------------------------
    // snake_case TS interface fields
    // -----------------------------------------------------------------------

    #[test]
    fn pages_use_snake_case_local_variable_names() {
        let r = synth_resource();
        // Check that reactive state variables use snake_case (e.g. submit_error not submitError)
        let create_body = build_create_page(&r);
        assert!(create_body.contains("submit_error"), "create page must use snake_case submit_error");
        let list_body = build_list_page(&r);
        assert!(list_body.contains("list_state"), "list page must use snake_case list_state");
        let edit_body = build_edit_page(&r);
        assert!(edit_body.contains("submit_error"), "edit page must use snake_case submit_error");
    }

    // -----------------------------------------------------------------------
    // Named routes (no hardcoded paths)
    // -----------------------------------------------------------------------

    #[test]
    fn pages_use_named_routes_not_hardcoded_paths() {
        let r = synth_resource();
        for (name, body) in [
            ("list", build_list_page(&r)),
            ("detail", build_detail_page(&r)),
            ("create", build_create_page(&r)),
            ("edit", build_edit_page(&r)),
        ] {
            assert!(
                !body.contains("router.push('/"),
                "{} page must not hardcode path strings",
                name
            );
            if body.contains("router.push") {
                assert!(
                    body.contains("ROUTE_NAMES["),
                    "{} page router.push must use ROUTE_NAMES",
                    name
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // pascal_label helper
    // -----------------------------------------------------------------------

    #[test]
    fn pascal_label_converts_table_names() {
        assert_eq!(pascal_label("users"), "Users");
        assert_eq!(pascal_label("order_items"), "Order Items");
        assert_eq!(pascal_label("widgets"), "Widgets");
    }
}
