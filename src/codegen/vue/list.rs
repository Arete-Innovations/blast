use crate::codegen::vue::naming::{pascal_case, singularize};
use crate::state::{FieldName, FieldState, FieldVariant, ListOptions, ResourceState, Verb};

pub fn build_list_sfc(r: &ResourceState) -> String {
    let table = r.name.as_str();
    let pascal = pascal_case(&singularize(table));

    let public_fields: Vec<(&FieldName, &FieldState)> = r
        .fields
        .iter()
        .filter(|(_, f)| f.variants.contains(&FieldVariant::Public))
        .collect();

    let list_options: Option<&ListOptions> = match r.verbs.get(&Verb::List) {
        Some(verb) => verb.list_options.as_ref(),
        None => return String::new(),
    };

    let columns_html = render_list_columns(&public_fields, list_options);
    let imports = build_list_imports(table);
    let paginator = render_paginator(list_options);
    let total_attr = if is_paginated(list_options) {
        " :total-records=\"totalRecords\""
    } else {
        ""
    };

    format!(
        "<script setup lang=\"ts\">\n\
{imports}\n\
const props = defineProps<{{\n\
  items: {pascal}Public[]\n\
  totalRecords?: number\n\
  loading?: boolean\n\
}}>()\n\
const emit = defineEmits<{{ requestPage: [query: ListQuery] }}>()\n\
\n\
const page = ref<number>(DEFAULT_PAGE)\n\
const pageSize = ref<number>(DEFAULT_PAGE_SIZE)\n\
const sortField = ref<string | undefined>(DEFAULT_SORT)\n\
const sortOrder = ref<number>(1)\n\
const filters = ref<Record<string, string>>({{}})\n\
const totalRecords = computed<number>(() => props.totalRecords ?? props.items.length)\n\
\n\
function buildQuery(): ListQuery {{\n\
  const sort = sortField.value === undefined\n\
    ? undefined\n\
    : `${{sortOrder.value < 0 ? '-' : '+'}}${{sortField.value}}`\n\
  return {{\n\
    page: page.value,\n\
    page_size: pageSize.value,\n\
    sort,\n\
    filters: {{ ...filters.value }},\n\
  }}\n\
}}\n\
\n\
function onPage(event: {{ page: number; rows: number }}): void {{\n\
  page.value = event.page + 1\n\
  pageSize.value = event.rows\n\
  emit('requestPage', buildQuery())\n\
}}\n\
\n\
function onSort(event: {{ sortField: string | undefined; sortOrder: number }}): void {{\n\
  sortField.value = event.sortField\n\
  sortOrder.value = event.sortOrder\n\
  emit('requestPage', buildQuery())\n\
}}\n\
\n\
function onFilter(col: string, value: string): void {{\n\
  if (value === '') {{\n\
    delete filters.value[col]\n\
  }} else {{\n\
    filters.value[col] = value\n\
  }}\n\
  emit('requestPage', buildQuery())\n\
}}\n\
</script>\n\
\n\
<template>\n\
  <DataTable\n\
    :value=\"items\"\n\
    :loading=\"loading === true\"\n\
    data-key=\"id\"\n\
    class=\"resource-list\"\n\
{paginator}\
{total_attr}\n\
    @page=\"onPage\"\n\
    @sort=\"onSort\"\n\
  >\n\
{columns}\
  </DataTable>\n\
</template>\n\
\n\
<style scoped>\n\
@layer app {{\n\
  .resource-list {{\n\
    display: block;\n\
    width: 100%;\n\
  }}\n\
}}\n\
</style>\n",
        imports = imports,
        pascal = pascal,
        columns = columns_html,
        paginator = paginator,
        total_attr = total_attr,
    )
}

fn build_list_imports(table: &str) -> String {
    let mut out = String::new();
    out.push_str("import { computed, ref } from 'vue'\n");
    out.push_str("import DataTable from 'primevue/datatable'\n");
    out.push_str("import Column from 'primevue/column'\n");
    out.push_str("import InputText from 'primevue/inputtext'\n");
    out.push_str(&format!(
        "import type {{ {pascal}Public }} from '@/generated/types/{table}'\n",
        pascal = pascal_case(&singularize(table)),
        table = table,
    ));
    out.push_str(
        "import {\n  DEFAULT_PAGE,\n  DEFAULT_PAGE_SIZE,\n  type ListQuery,\n} from '@/generated/queries/list_query'\n",
    );
    out.push_str(&format!(
        "import {{ DEFAULT_SORT, isFilterable, isSortable }} from '@/generated/queries/{table}_list'\n",
        table = table
    ));
    out.push_str(
        "// keep helpers referenced for downstream consumers and tree-shake friendliness\n",
    );
    out.push_str("void isFilterable\n");
    out.push_str("void isSortable\n");
    out
}

fn is_paginated(opts: Option<&ListOptions>) -> bool {
    opts.is_some_and(|o| o.paginated)
}

fn render_paginator(opts: Option<&ListOptions>) -> String {
    if !is_paginated(opts) {
        return String::new();
    }
    String::from(
        "    :paginator=\"true\"\n    :rows=\"pageSize\"\n    :rows-per-page-options=\"[10, 25, 50, 100]\"\n    lazy\n",
    )
}

fn render_list_columns(
    fields: &[(&FieldName, &FieldState)],
    opts: Option<&ListOptions>,
) -> String {
    let mut out = String::new();
    for (name, _f) in fields {
        let col = name.as_str();
        let sortable = column_sortable(opts, col);
        let filterable = column_filterable(opts, col);
        let sort_attr = if sortable { " :sortable=\"true\"" } else { "" };
        let filter_slot = if filterable {
            format!(
                "      <template #filter>\n        <InputText placeholder=\"Filter {col}\" @input=\"(e) => onFilter('{col}', (e.target as HTMLInputElement).value)\" />\n      </template>\n",
                col = col
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            "    <Column field=\"{col}\" header=\"{col}\"{sort_attr}>\n{filter_slot}    </Column>\n",
            col = col,
            sort_attr = sort_attr,
            filter_slot = filter_slot,
        ));
    }
    out
}

fn column_sortable(opts: Option<&ListOptions>, col: &str) -> bool {
    opts.is_some_and(|o| o.sortable_columns.iter().any(|c| c.as_str() == col))
}

fn column_filterable(opts: Option<&ListOptions>, col: &str) -> bool {
    opts.is_some_and(|o| o.filterable_columns.iter().any(|c| c.as_str() == col))
}
