//! TS source rendering for per-resource composables.
//!
//! The body of every `<resource>.ts` file is assembled here as a single
//! `String`. Imports and the public surface are conditionally narrowed
//! based on which verbs the Primer enables — a Primer with only `List`
//! emits `useResourcesList` and nothing else, etc.
//!
//! Rules carried in (Governor):
//! - No `: any`, `as any`. Strict TS.
//! - No `console.log`.
//! - No literal `||` / `??` fallbacks. Use explicit conditionals.
//! - No `new WebSocket(`; subscribe via `useChannel`.
//! - No raw `fetch(`; route through `@/generated/api/<resource>`.
//! - snake_case for backend interface fields (we re-export typed
//!   interfaces from `@/generated/types/<resource>` — those already
//!   carry snake_case by codegen).
//! - camelCase for FE framework identifiers (composables, opts).

use crate::codegen::composables_v2::naming::{bus_prefix, plural_pascal, singular_pascal};
use crate::state::{ResourceState, TopicScope, Verb};

/// Build the full TS body (no marker — caller prepends it).
pub fn build_resource_ts(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let singular = singular_pascal(resource);
    let plural = plural_pascal(table);

    let has_list = resource.verbs.contains_key(&Verb::List);
    let has_get = resource.verbs.contains_key(&Verb::Get);
    let has_create = resource.verbs.contains_key(&Verb::Create);
    let has_update = resource.verbs.contains_key(&Verb::Update);
    let has_delete = resource.verbs.contains_key(&Verb::Delete);

    let imports = build_imports(
        table, &singular, has_list, has_get, has_create, has_update, has_delete,
    );

    let mut body = String::new();
    body.push_str(&imports);
    body.push('\n');

    if has_list {
        body.push_str(&render_list_composable(resource, &singular, &plural));
        body.push('\n');
    }
    if has_get {
        body.push_str(&render_single_composable(resource, &singular));
        body.push('\n');
    }
    if has_create {
        body.push_str(&render_create_composable(table, &singular));
        body.push('\n');
    }
    if has_update {
        body.push_str(&render_update_composable(table, &singular));
        body.push('\n');
    }
    if has_delete {
        body.push_str(&render_delete_composable(table, &singular));
        body.push('\n');
    }

    body
}

fn build_imports(
    table: &str,
    singular: &str,
    has_list: bool,
    has_get: bool,
    has_create: bool,
    has_update: bool,
    has_delete: bool,
) -> String {
    let mut vue_imports: Vec<&str> = Vec::new();
    if has_list || has_get {
        vue_imports.push("onMounted");
        vue_imports.push("onUnmounted");
        vue_imports.push("ref");
        vue_imports.push("watch");
    }
    if has_list {
        vue_imports.push("watchEffect");
    }
    if has_get {
        vue_imports.push("computed");
        vue_imports.push("isRef");
    }

    let mut type_imports: Vec<String> = Vec::new();
    if has_list || has_get {
        type_imports.push(format!("{}Public", singular));
    }
    if has_create {
        type_imports.push(format!("{}Insertable", singular));
    }
    if has_update {
        type_imports.push(format!("{}Patch", singular));
    }
    if has_list {
        type_imports.push(format!("{}Filter", singular));
    }

    let mut api_imports: Vec<&str> = Vec::new();
    if has_list {
        api_imports.push("listResource");
    }
    if has_get {
        api_imports.push("getResource");
    }
    if has_create {
        api_imports.push("createResource");
    }
    if has_update {
        api_imports.push("updateResource");
    }
    if has_delete {
        api_imports.push("deleteResource");
    }

    let needs_get_ref = has_get;
    let mut out = String::new();
    let vue_dedup = dedup_keep_order(vue_imports.iter().copied());
    if !vue_dedup.is_empty() || needs_get_ref {
        let mut all: Vec<String> = vue_dedup.iter().map(|s| s.to_string()).collect();
        if needs_get_ref && !all.iter().any(|n| n == "Ref") {
            all.push("Ref".to_string());
        }
        out.push_str(&format!(
            "import {{ {names} }} from 'vue'\n",
            names = all.join(", ")
        ));
    }

    out.push_str("import type { MeltDownResponse } from '@/generated/types/meltdown'\n");

    if !type_imports.is_empty() {
        out.push_str(&format!(
            "import type {{ {names} }} from '@/generated/types/{table}'\n",
            names = type_imports.join(", "),
            table = table,
        ));
    }

    if !api_imports.is_empty() {
        let aliased: Vec<String> = api_imports
            .iter()
            .map(|name| api_alias(name, singular))
            .collect();
        out.push_str(&format!(
            "import {{ {names} }} from '@/generated/api/{table}'\n",
            names = aliased.join(", "),
            table = table,
        ));
    }

    if has_list {
        out.push_str("import { useUrlListState } from '@/composables/url'\n");
    }
    if has_list || has_get {
        out.push_str("import { useChannel } from '@/composables/channel'\n");
        out.push_str(
            "import { getNavAbortSignal } from '@/router/install-blocking-nav'\n",
        );
    }

    out.push_str("import { emit, on } from '@/generated/bus'\n");

    out
}

fn api_alias(name: &str, singular: &str) -> String {
    // `listResource` is a placeholder — the actual API client per Primer
    // exports `list<Resource>` / `get<Resource>` / `create<Resource>` /
    // `update<Resource>` / `delete<Resource>` (camelCase). We import
    // those by their real names; the alias lookup below maps the
    // placeholder we used for ordering back to the real symbol.
    let suffix = singular;
    match name {
        "listResource" => format!("list{}s as apiList", suffix),
        "getResource" => format!("get{} as apiGet", suffix),
        "createResource" => format!("create{} as apiCreate", suffix),
        "updateResource" => format!("update{} as apiUpdate", suffix),
        "deleteResource" => format!("delete{} as apiDelete", suffix),
        _other => name.to_string(),
    }
}

fn render_list_composable(resource: &ResourceState, singular: &str, plural: &str) -> String {
    let table = resource.name.as_str();
    let bus = bus_prefix(table);
    let topic = list_topic(resource);

    format!(
        "export interface Use{plural}ListOpts {{\n\
  poll?: number\n\
  live?: boolean\n\
  filter?: {singular}Filter\n\
}}\n\
\n\
export function use{plural}List(opts: Use{plural}ListOpts = {{}}) {{\n\
  const data = ref<{singular}Public[] | null>(null)\n\
  const error = ref<MeltDownResponse | null>(null)\n\
  const url = useUrlListState()\n\
\n\
  let pendingController: AbortController | null = null\n\
  const refetch = async (): Promise<void> => {{\n\
    if (pendingController !== null) {{\n\
      pendingController.abort()\n\
    }}\n\
    const controller = new AbortController()\n\
    pendingController = controller\n\
    const navSignal = getNavAbortSignal()\n\
    if (navSignal !== null) {{\n\
      navSignal.addEventListener('abort', () => controller.abort(), {{ once: true }})\n\
    }}\n\
    const filterValue = opts.filter === undefined ? url.filter.value : opts.filter\n\
    const result = await apiList(\n\
      {{\n\
        page: url.page.value,\n\
        page_size: url.pageSize.value,\n\
        sort: url.sort.value,\n\
        filter: filterValue,\n\
      }},\n\
      controller.signal,\n\
    )\n\
    if (controller.signal.aborted) {{\n\
      return\n\
    }}\n\
    if (result.data !== null) {{\n\
      data.value = result.data\n\
      error.value = null\n\
    }} else {{\n\
      error.value = result.error\n\
    }}\n\
  }}\n\
\n\
  watchEffect(() => {{\n\
    void url.page.value\n\
    void url.pageSize.value\n\
    void url.sort.value\n\
    void url.filter.value\n\
    void refetch()\n\
  }})\n\
\n\
  let pollTimer: ReturnType<typeof setInterval> | null = null\n\
  if (opts.poll !== undefined && opts.poll > 0) {{\n\
    onMounted(() => {{\n\
      pollTimer = setInterval(() => {{\n\
        if (document.visibilityState === 'visible') {{\n\
          void refetch()\n\
        }}\n\
      }}, opts.poll)\n\
    }})\n\
    onUnmounted(() => {{\n\
      if (pollTimer !== null) {{\n\
        clearInterval(pollTimer)\n\
        pollTimer = null\n\
      }}\n\
    }})\n\
  }}\n\
\n\
  if (opts.live === true) {{\n\
    const channel = useChannel('{topic}')\n\
    watch(channel.lastEvent, () => {{\n\
      void refetch()\n\
    }})\n\
  }}\n\
\n\
  let unsubCreated: (() => void) | null = null\n\
  let unsubUpdated: (() => void) | null = null\n\
  let unsubDeleted: (() => void) | null = null\n\
  onMounted(() => {{\n\
    unsubCreated = on('{bus}:created', () => {{ void refetch() }})\n\
    unsubUpdated = on('{bus}:updated', () => {{ void refetch() }})\n\
    unsubDeleted = on('{bus}:deleted', () => {{ void refetch() }})\n\
  }})\n\
  onUnmounted(() => {{\n\
    if (unsubCreated !== null) {{ unsubCreated() }}\n\
    if (unsubUpdated !== null) {{ unsubUpdated() }}\n\
    if (unsubDeleted !== null) {{ unsubDeleted() }}\n\
  }})\n\
\n\
  return {{\n\
    data,\n\
    error,\n\
    refetch,\n\
    page: url.page,\n\
    pageSize: url.pageSize,\n\
    sort: url.sort,\n\
    filter: url.filter,\n\
  }}\n\
}}\n",
        plural = plural,
        singular = singular,
        topic = topic,
        bus = bus,
    )
}

fn render_single_composable(resource: &ResourceState, singular: &str) -> String {
    let table = resource.name.as_str();
    let bus = bus_prefix(table);
    let per_row_topic = single_topic(resource);
    let live_block = render_single_live_block(per_row_topic.as_deref());

    format!(
        "export function use{singular}(id: Ref<number> | number) {{\n\
  const data = ref<{singular}Public | null>(null)\n\
  const error = ref<MeltDownResponse | null>(null)\n\
\n\
  let pendingController: AbortController | null = null\n\
  const refetch = async (): Promise<void> => {{\n\
    if (pendingController !== null) {{\n\
      pendingController.abort()\n\
    }}\n\
    const controller = new AbortController()\n\
    pendingController = controller\n\
    const navSignal = getNavAbortSignal()\n\
    if (navSignal !== null) {{\n\
      navSignal.addEventListener('abort', () => controller.abort(), {{ once: true }})\n\
    }}\n\
    const idValue: number = isRef(id) ? id.value : id\n\
    const result = await apiGet(idValue, controller.signal)\n\
    if (controller.signal.aborted) {{\n\
      return\n\
    }}\n\
    if (result.data !== null) {{\n\
      data.value = result.data\n\
      error.value = null\n\
    }} else {{\n\
      error.value = result.error\n\
    }}\n\
  }}\n\
\n\
  watch(\n\
    () => (isRef(id) ? id.value : id),\n\
    () => {{ void refetch() }},\n\
    {{ immediate: true }},\n\
  )\n\
\n\
{live_block}\
  let unsubUpdated: (() => void) | null = null\n\
  let unsubDeleted: (() => void) | null = null\n\
  onMounted(() => {{\n\
    unsubUpdated = on('{bus}:updated', () => {{ void refetch() }})\n\
    unsubDeleted = on('{bus}:deleted', () => {{ void refetch() }})\n\
  }})\n\
  onUnmounted(() => {{\n\
    if (unsubUpdated !== null) {{ unsubUpdated() }}\n\
    if (unsubDeleted !== null) {{ unsubDeleted() }}\n\
  }})\n\
\n\
  return {{ data, error, refetch }}\n\
}}\n",
        singular = singular,
        bus = bus,
        live_block = live_block,
    )
}

fn render_create_composable(table: &str, singular: &str) -> String {
    let bus = bus_prefix(table);
    format!(
        "export function useCreate{singular}() {{\n\
  return async (\n\
    input: {singular}Insertable,\n\
  ): Promise<{{ data: {singular}Public | null; error: MeltDownResponse | null }}> => {{\n\
    const result = await apiCreate(input)\n\
    if (result.data !== null) {{\n\
      emit('{bus}:created', result.data)\n\
    }}\n\
    return result\n\
  }}\n\
}}\n",
        singular = singular,
        bus = bus,
    )
}

fn render_update_composable(table: &str, singular: &str) -> String {
    let bus = bus_prefix(table);
    format!(
        "export function useUpdate{singular}() {{\n\
  return async (\n\
    id: number,\n\
    patch: {singular}Patch,\n\
  ): Promise<{{ data: {singular}Public | null; error: MeltDownResponse | null }}> => {{\n\
    const result = await apiUpdate(id, patch)\n\
    if (result.data !== null) {{\n\
      emit('{bus}:updated', result.data)\n\
    }}\n\
    return result\n\
  }}\n\
}}\n",
        singular = singular,
        bus = bus,
    )
}

fn render_delete_composable(table: &str, singular: &str) -> String {
    let bus = bus_prefix(table);
    format!(
        "export function useDelete{singular}() {{\n\
  return async (\n\
    id: number,\n\
  ): Promise<{{ data: {{ id: number }} | null; error: MeltDownResponse | null }}> => {{\n\
    const result = await apiDelete(id)\n\
    if (result.data !== null) {{\n\
      emit('{bus}:deleted', {{ id }})\n\
    }}\n\
    return result\n\
  }}\n\
}}\n",
        singular = singular,
        bus = bus,
    )
}

/// Render the WS-live block of `useResource`. An absent topic means
/// the Primer doesn't expose per-row WS — emit nothing in that case,
/// which is a deliberate rendering choice (the bus invalidation block
/// elsewhere keeps the composable in sync via mutation events).
fn render_single_live_block(per_row_topic: Option<&str>) -> String {
    let mut out = String::new();
    match per_row_topic {
        Some(topic_template) => out.push_str(&format!(
            "  const idForTopic = computed<number>(() => isRef(id) ? id.value : id)\n\
  const channel = useChannel(`{topic_template}`)\n\
  watch(channel.lastEvent, () => {{ void refetch() }})\n\
  void idForTopic\n",
            topic_template = topic_template,
        )),
        None => {}
    }
    out
}

/// List-wide WS topic. We always emit `<table>/all` for list
/// composables; per-row scopes change the single-resource topic but
/// the list-wide channel is the same.
fn list_topic(resource: &ResourceState) -> String {
    format!("{}/all", resource.name.as_str())
}

/// Single-resource WS topic template (interpolated client-side via
/// `idForTopic`). Returns `Some` only when the Primer scope is
/// `PerRow` — `Global` and `ScopedTo` topic shapes don't carry per-row
/// identity so the single-resource composable falls back to
/// bus-driven invalidation. Builds the result via explicit branches
/// (no error-arm-style fallbacks).
fn single_topic(resource: &ResourceState) -> Option<String> {
    let events = resource.ws_events.as_ref()?;
    if let TopicScope::PerRow = events.topic_scope {
        return Some(format!("{}/${{idForTopic.value}}", resource.name.as_str()));
    }
    Option::None
}

fn dedup_keep_order<'a, I: IntoIterator<Item = &'a str>>(items: I) -> Vec<&'a str> {
    let mut seen: Vec<&'a str> = Vec::new();
    for item in items {
        if !seen.iter().any(|existing| *existing == item) {
            seen.push(item);
        }
    }
    seen
}
