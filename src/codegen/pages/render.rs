//! Vue SFC bodies for per-resource CRUD pages.
//!
//! Self-contained: each page imports only `@/generated/types/<r>` and
//! `@/generated/api/<r>`. No composable layer, no shared component layer,
//! no router-from-state coupling. Routes are user-wired via
//! `frontend/src/custom/router.ts`.

use crate::codegen::structs::naming::type_stem_for_resource;
use crate::state::{FieldVariant, ResourceState, Verb};

pub fn build_list_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let label = pascal_label(table);
    let public_cols: Vec<&str> = r
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Public))
        .map(|(name, _)| name.as_str())
        .collect();

    let columns_html = public_cols
        .iter()
        .map(|c| format!("        <Column field=\"{c}\" header=\"{c}\" />"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<script setup lang="ts">
import {{ onMounted, ref }} from 'vue'
import DataTable from 'primevue/datatable'
import Column from 'primevue/column'
import {{ list{stem}s }} from '@/generated/api/{table}'
import type {{ {stem}Public }} from '@/generated/types/{table}'

const items = ref<{stem}Public[]>([])
const loading = ref<boolean>(false)
const error_message = ref<string | null>(null)

async function load(): Promise<void> {{
  loading.value = true
  error_message.value = null
  const result = await list{stem}s({{ page: 1, page_size: 50 }})
  loading.value = false
  if (result.error !== null) {{
    error_message.value = result.error.error.message
    return
  }}
  items.value = result.data === null ? [] : result.data
}}

onMounted(load)
</script>

<template>
  <section class="{table}-list-page">
    <header class="{table}-list-header">
      <h1>{label}</h1>
    </header>
    <div v-if="error_message !== null" class="{table}-list-error" role="alert">
      {{{{ error_message }}}}
    </div>
    <DataTable :value="items" :loading="loading" data-key="id" striped-rows>
{columns_html}
    </DataTable>
  </section>
</template>

<style scoped>
@layer app {{
  .{table}-list-page {{
    display: flex;
    flex-direction: column;
    gap: var(--app-space-lg);
    padding: var(--app-space-lg);
  }}
  .{table}-list-header {{
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }}
  .{table}-list-error {{
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));
  }}
}}
</style>
"#,
        stem = stem,
        table = table,
        label = label,
        columns_html = columns_html,
    )
}

pub fn build_detail_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let label = pascal_label(table);

    format!(
        r#"<script setup lang="ts">
import {{ onMounted, ref, watch }} from 'vue'
import {{ get{stem} }} from '@/generated/api/{table}'
import type {{ {stem}Public }} from '@/generated/types/{table}'

const props = defineProps<{{ id: number }}>()

const item = ref<{stem}Public | null>(null)
const loading = ref<boolean>(false)
const error_message = ref<string | null>(null)

async function load(id: number): Promise<void> {{
  loading.value = true
  error_message.value = null
  const result = await get{stem}(id)
  loading.value = false
  if (result.error !== null) {{
    error_message.value = result.error.error.message
    return
  }}
  item.value = result.data
}}

onMounted(() => {{ void load(props.id) }})
watch(() => props.id, (next) => {{ void load(next) }})
</script>

<template>
  <section class="{table}-detail-page">
    <header class="{table}-detail-header">
      <h1>{label}</h1>
    </header>
    <div v-if="error_message !== null" class="{table}-detail-error" role="alert">
      {{{{ error_message }}}}
    </div>
    <pre v-if="item !== null" class="{table}-detail-json">{{{{ JSON.stringify(item, null, 2) }}}}</pre>
  </section>
</template>

<style scoped>
@layer app {{
  .{table}-detail-page {{
    display: flex;
    flex-direction: column;
    gap: var(--app-space-lg);
    padding: var(--app-space-lg);
  }}
  .{table}-detail-error {{
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));
  }}
  .{table}-detail-json {{
    background: var(--p-content-background, transparent);
    padding: var(--app-space-md);
    border-radius: var(--app-radius-md);
    overflow: auto;
  }}
}}
</style>
"#,
        stem = stem,
        table = table,
        label = label,
    )
}

pub fn build_create_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let label = pascal_label(table);

    format!(
        r#"<script setup lang="ts">
import {{ ref }} from 'vue'
import {{ useRouter }} from 'vue-router'
import Button from 'primevue/button'
import {{ create{stem} }} from '@/generated/api/{table}'
import type {{ {stem}Insertable }} from '@/generated/types/{table}'

const router = useRouter()
const submitting = ref<boolean>(false)
const error_message = ref<string | null>(null)
const draft = ref<string>('{{}}')

async function on_submit(): Promise<void> {{
  submitting.value = true
  error_message.value = null
  let payload: {stem}Insertable
  try {{
    payload = JSON.parse(draft.value) as {stem}Insertable
  }} catch (_e) {{
    submitting.value = false
    error_message.value = 'Invalid JSON in draft body.'
    return
  }}
  const result = await create{stem}(payload)
  submitting.value = false
  if (result.error !== null) {{
    error_message.value = result.error.error.message
    return
  }}
  router.back()
}}
</script>

<template>
  <section class="{table}-create-page">
    <header class="{table}-create-header">
      <h1>New {label}</h1>
    </header>
    <div v-if="error_message !== null" class="{table}-create-error" role="alert">
      {{{{ error_message }}}}
    </div>
    <textarea
      v-model="draft"
      class="{table}-create-draft"
      rows="12"
      aria-label="JSON draft for new {table} record"
    ></textarea>
    <Button
      label="Create"
      :loading="submitting"
      :disabled="submitting"
      @click="on_submit"
    />
  </section>
</template>

<style scoped>
@layer app {{
  .{table}-create-page {{
    display: flex;
    flex-direction: column;
    gap: var(--app-space-lg);
    padding: var(--app-space-lg);
    max-width: 720px;
  }}
  .{table}-create-error {{
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));
  }}
  .{table}-create-draft {{
    font-family: var(--app-font-mono, monospace);
    padding: var(--app-space-md);
    border-radius: var(--app-radius-md);
    border: 1px solid var(--p-content-border-color, transparent);
  }}
}}
</style>
"#,
        stem = stem,
        table = table,
        label = label,
    )
}

pub fn build_edit_page(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let stem = type_stem_for_resource(r);
    let label = pascal_label(table);

    format!(
        r#"<script setup lang="ts">
import {{ onMounted, ref }} from 'vue'
import {{ useRouter }} from 'vue-router'
import Button from 'primevue/button'
import {{ get{stem}, update{stem} }} from '@/generated/api/{table}'
import type {{ {stem}Patch, {stem}Public }} from '@/generated/types/{table}'

const props = defineProps<{{ id: number }}>()
const router = useRouter()

const original = ref<{stem}Public | null>(null)
const draft = ref<string>('{{}}')
const submitting = ref<boolean>(false)
const error_message = ref<string | null>(null)

async function load(): Promise<void> {{
  const result = await get{stem}(props.id)
  if (result.error !== null) {{
    error_message.value = result.error.error.message
    return
  }}
  original.value = result.data
  draft.value = JSON.stringify(result.data, null, 2)
}}

async function on_submit(): Promise<void> {{
  submitting.value = true
  error_message.value = null
  let patch: {stem}Patch
  try {{
    patch = JSON.parse(draft.value) as {stem}Patch
  }} catch (_e) {{
    submitting.value = false
    error_message.value = 'Invalid JSON in draft body.'
    return
  }}
  const result = await update{stem}(props.id, patch)
  submitting.value = false
  if (result.error !== null) {{
    error_message.value = result.error.error.message
    return
  }}
  router.back()
}}

onMounted(load)
</script>

<template>
  <section class="{table}-edit-page">
    <header class="{table}-edit-header">
      <h1>Edit {label} #{{{{ id }}}}</h1>
    </header>
    <div v-if="error_message !== null" class="{table}-edit-error" role="alert">
      {{{{ error_message }}}}
    </div>
    <textarea
      v-model="draft"
      class="{table}-edit-draft"
      rows="12"
      aria-label="JSON patch body for {table}"
    ></textarea>
    <Button
      label="Save"
      :loading="submitting"
      :disabled="submitting || original === null"
      @click="on_submit"
    />
  </section>
</template>

<style scoped>
@layer app {{
  .{table}-edit-page {{
    display: flex;
    flex-direction: column;
    gap: var(--app-space-lg);
    padding: var(--app-space-lg);
    max-width: 720px;
  }}
  .{table}-edit-error {{
    color: var(--p-message-error-color, var(--app-color-danger, #b00020));
  }}
  .{table}-edit-draft {{
    font-family: var(--app-font-mono, monospace);
    padding: var(--app-space-md);
    border-radius: var(--app-radius-md);
    border: 1px solid var(--p-content-border-color, transparent);
  }}
}}
</style>
"#,
        stem = stem,
        table = table,
        label = label,
    )
}

pub fn pages_for_resource(r: &ResourceState) -> Vec<(String, String)> {
    let stem = type_stem_for_resource(r);
    let mut out: Vec<(String, String)> = Vec::new();
    if r.verbs.contains_key(&Verb::List) {
        out.push((format!("{stem}ListPage.vue"), build_list_page(r)));
    }
    if r.verbs.contains_key(&Verb::Get) {
        out.push((format!("{stem}DetailPage.vue"), build_detail_page(r)));
    }
    if r.verbs.contains_key(&Verb::Create) {
        out.push((format!("{stem}CreatePage.vue"), build_create_page(r)));
    }
    if r.verbs.contains_key(&Verb::Update) {
        out.push((format!("{stem}EditPage.vue"), build_edit_page(r)));
    }
    out
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
