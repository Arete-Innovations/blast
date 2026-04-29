use crate::{
    codegen::structs::naming::type_stem_for_resource,
    state::{ResourceState, Verb, WsEventsState},
};

pub fn build_resource_composables(resource: &ResourceState) -> String {
    let table = resource.name.as_str();
    let singular = type_stem_for_resource(resource);
    let plural = plural_of_pascal(&singular, table);

    let has_list = resource.verbs.contains_key(&Verb::List);
    let has_get = resource.verbs.contains_key(&Verb::Get);
    let has_create = resource.verbs.contains_key(&Verb::Create);
    let has_update = resource.verbs.contains_key(&Verb::Update);
    let has_delete = resource.verbs.contains_key(&Verb::Delete);

    let needs_url_state = has_list;
    let needs_channel = has_list && resource.ws_events.is_some();

    let mut out = String::new();

    let mut vue_imports: Vec<&'static str> = Vec::new();
    vue_imports.push("ref");
    if has_list {
        vue_imports.push("watch");
    }
    if has_get {
        vue_imports.push("watch");
    }
    if has_list || has_get {
        vue_imports.push("onMounted");
        vue_imports.push("onUnmounted");
    }
    let mut seen_vue: Vec<&'static str> = Vec::new();
    for sym in vue_imports.iter() {
        if !seen_vue.contains(sym) {
            seen_vue.push(*sym);
        }
    }
    out.push_str(&format!("import {{ {imports} }} from 'vue'\n", imports = seen_vue.join(", ")));
    out.push_str("import type { Ref } from 'vue'\n");

    if needs_url_state {
        out.push_str("import { useUrlListState } from '@/composables/url'\n");
    }
    if needs_channel {
        out.push_str("import { useChannel } from '@/composables/channel'\n");
    }

    let mut api_imports: Vec<String> = Vec::new();
    if has_list {
        api_imports.push(format!("list{plural}"));
    }
    if has_get {
        api_imports.push(format!("get{singular}"));
    }
    if has_create {
        api_imports.push(format!("create{singular}"));
    }
    if has_update {
        api_imports.push(format!("update{singular}"));
    }
    if has_delete {
        api_imports.push(format!("delete{singular}"));
    }
    if !api_imports.is_empty() {
        out.push_str(&format!("import {{ {names} }} from '@/generated/api/{table}'\n", names = api_imports.join(", "), table = table));
    }

    let mut type_imports: Vec<String> = Vec::new();
    if has_list || has_get || has_create || has_update {
        type_imports.push(format!("{singular}Public"));
    }
    if has_create {
        type_imports.push(format!("{singular}Insertable"));
    }
    if has_update {
        type_imports.push(format!("{singular}Patch"));
    }
    if !type_imports.is_empty() {
        out.push_str(&format!("import type {{ {names} }} from '@/generated/types/{table}'\n", names = type_imports.join(", "), table = table));
    }

    out.push_str("import type { MeltDownResponse } from '@/generated/types/meltdown'\n");
    out.push('\n');

    if has_list {
        out.push_str("export interface UseListOpts {\n");
        out.push_str("  poll?: number\n");
        out.push_str("  live?: boolean\n");
        out.push_str("}\n\n");
    }

    if has_list {
        out.push_str(&render_list_composable(table, &singular, &plural, resource.ws_events.as_ref()));
        out.push('\n');
    }
    if has_get {
        out.push_str(&render_item_composable(&singular));
        out.push('\n');
    }
    if has_create {
        out.push_str(&render_create_composable(&singular));
        out.push('\n');
    }
    if has_update {
        out.push_str(&render_update_composable(&singular));
        out.push('\n');
    }
    if has_delete {
        out.push_str(&render_delete_composable(&singular));
        out.push('\n');
    }

    let trimmed = out.trim_end_matches('\n');
    format!("{trimmed}\n")
}

fn render_list_composable(table: &str, singular: &str, plural: &str, ws: Option<&WsEventsState>) -> String {
    let live_block = if ws.is_some() {
        format!(
            "  if (opts !== undefined && opts.live === true) {{\n    useChannel<unknown>('{table}.changed', {{ onMessage: () => {{ void refetch() }} }})\n  }}\n",
            table = table
        )
    } else {
        String::new()
    };

    let mut body = String::new();
    body.push_str(&format!("export function use{plural}List(opts?: UseListOpts): {{\n"));
    body.push_str(&format!("  data: Ref<{singular}Public[] | null>\n"));
    body.push_str("  error: Ref<MeltDownResponse | null>\n");
    body.push_str("  loading: Ref<boolean>\n");
    body.push_str("  refetch: () => Promise<void>\n");
    body.push_str("  page: ReturnType<typeof useUrlListState>['page']\n");
    body.push_str("  pageSize: ReturnType<typeof useUrlListState>['pageSize']\n");
    body.push_str("  sort: ReturnType<typeof useUrlListState>['sort']\n");
    body.push_str("  filter: ReturnType<typeof useUrlListState>['filter']\n");
    body.push_str("  total: Ref<number>\n");
    body.push_str("  total_pages: Ref<number>\n");
    body.push_str("} {\n");
    body.push_str(&format!("  const data = ref<{singular}Public[] | null>(null) as Ref<{singular}Public[] | null>\n"));
    body.push_str("  const error = ref<MeltDownResponse | null>(null) as Ref<MeltDownResponse | null>\n");
    body.push_str("  const loading = ref<boolean>(false)\n");
    body.push_str("  const total = ref<number>(0)\n");
    body.push_str("  const total_pages = ref<number>(0)\n");
    body.push_str("  const { page, pageSize, sort, filter } = useUrlListState()\n\n");
    body.push_str("  let in_flight: AbortController | null = null\n");
    body.push_str("  let poll_handle: number | null = null\n");
    body.push_str("  let poll_ms: number = 0\n");
    body.push_str("  if (opts !== undefined && opts.poll !== undefined && opts.poll > 0) {\n");
    body.push_str("    poll_ms = opts.poll\n");
    body.push_str("  }\n\n");
    body.push_str("  async function refetch(): Promise<void> {\n");
    body.push_str("    if (in_flight !== null) {\n");
    body.push_str("      in_flight.abort()\n");
    body.push_str("    }\n");
    body.push_str("    const controller = new AbortController()\n");
    body.push_str("    in_flight = controller\n");
    body.push_str("    loading.value = true\n");
    body.push_str(&format!("    const result = await list{plural}({{\n"));
    body.push_str("      page: page.value,\n");
    body.push_str("      page_size: pageSize.value,\n");
    body.push_str("      sort: sort.value === '' ? null : sort.value,\n");
    body.push_str("      filter: filter.value as { [key: string]: string | number | boolean | null | undefined },\n");
    body.push_str("    }, controller.signal)\n");
    body.push_str("    loading.value = false\n");
    body.push_str("    if (in_flight === controller) {\n");
    body.push_str("      in_flight = null\n");
    body.push_str("    }\n");
    body.push_str("    if (controller.signal.aborted === true) {\n");
    body.push_str("      return\n");
    body.push_str("    }\n");
    body.push_str("    if (result.error !== null) {\n");
    body.push_str("      error.value = result.error\n");
    body.push_str("      return\n");
    body.push_str("    }\n");
    body.push_str("    error.value = null\n");
    body.push_str("    data.value = result.data\n");
    body.push_str("    total.value = result.total\n");
    body.push_str("    total_pages.value = result.total_pages\n");
    body.push_str("  }\n\n");
    body.push_str("  function start_polling(): void {\n");
    body.push_str("    if (poll_ms <= 0) {\n");
    body.push_str("      return\n");
    body.push_str("    }\n");
    body.push_str("    if (poll_handle !== null) {\n");
    body.push_str("      return\n");
    body.push_str("    }\n");
    body.push_str("    poll_handle = window.setInterval(() => { void refetch() }, poll_ms)\n");
    body.push_str("  }\n\n");
    body.push_str("  function stop_polling(): void {\n");
    body.push_str("    if (poll_handle === null) {\n");
    body.push_str("      return\n");
    body.push_str("    }\n");
    body.push_str("    window.clearInterval(poll_handle)\n");
    body.push_str("    poll_handle = null\n");
    body.push_str("  }\n\n");
    body.push_str("  function on_visibility_change(): void {\n");
    body.push_str("    if (document.visibilityState === 'hidden') {\n");
    body.push_str("      stop_polling()\n");
    body.push_str("    } else {\n");
    body.push_str("      start_polling()\n");
    body.push_str("    }\n");
    body.push_str("  }\n\n");
    body.push_str(&live_block);
    body.push_str("  watch([page, pageSize, sort, filter], () => { void refetch() }, { deep: true })\n\n");
    body.push_str("  onMounted(() => {\n");
    body.push_str("    void refetch()\n");
    body.push_str("    if (poll_ms > 0) {\n");
    body.push_str("      start_polling()\n");
    body.push_str("      document.addEventListener('visibilitychange', on_visibility_change)\n");
    body.push_str("    }\n");
    body.push_str("  })\n\n");
    body.push_str("  onUnmounted(() => {\n");
    body.push_str("    if (in_flight !== null) {\n");
    body.push_str("      in_flight.abort()\n");
    body.push_str("      in_flight = null\n");
    body.push_str("    }\n");
    body.push_str("    stop_polling()\n");
    body.push_str("    if (poll_ms > 0) {\n");
    body.push_str("      document.removeEventListener('visibilitychange', on_visibility_change)\n");
    body.push_str("    }\n");
    body.push_str("  })\n\n");
    body.push_str("  return { data, error, loading, refetch, page, pageSize, sort, filter, total, total_pages }\n");
    body.push_str("}\n");
    body
}

fn render_item_composable(singular: &str) -> String {
    let mut body = String::new();
    body.push_str(&format!("export function use{singular}(id: Ref<number>): {{\n"));
    body.push_str(&format!("  data: Ref<{singular}Public | null>\n"));
    body.push_str("  error: Ref<MeltDownResponse | null>\n");
    body.push_str("  loading: Ref<boolean>\n");
    body.push_str("  refetch: () => Promise<void>\n");
    body.push_str("} {\n");
    body.push_str(&format!("  const data = ref<{singular}Public | null>(null) as Ref<{singular}Public | null>\n"));
    body.push_str("  const error = ref<MeltDownResponse | null>(null) as Ref<MeltDownResponse | null>\n");
    body.push_str("  const loading = ref<boolean>(false)\n\n");
    body.push_str("  let in_flight: AbortController | null = null\n\n");
    body.push_str("  async function refetch(): Promise<void> {\n");
    body.push_str("    if (in_flight !== null) {\n");
    body.push_str("      in_flight.abort()\n");
    body.push_str("    }\n");
    body.push_str("    const controller = new AbortController()\n");
    body.push_str("    in_flight = controller\n");
    body.push_str("    loading.value = true\n");
    body.push_str(&format!("    const result = await get{singular}(id.value, controller.signal)\n"));
    body.push_str("    loading.value = false\n");
    body.push_str("    if (in_flight === controller) {\n");
    body.push_str("      in_flight = null\n");
    body.push_str("    }\n");
    body.push_str("    if (controller.signal.aborted === true) {\n");
    body.push_str("      return\n");
    body.push_str("    }\n");
    body.push_str("    if (result.error !== null) {\n");
    body.push_str("      error.value = result.error\n");
    body.push_str("      return\n");
    body.push_str("    }\n");
    body.push_str("    error.value = null\n");
    body.push_str("    data.value = result.data\n");
    body.push_str("  }\n\n");
    body.push_str("  watch(id, () => { void refetch() })\n\n");
    body.push_str("  onMounted(() => { void refetch() })\n\n");
    body.push_str("  onUnmounted(() => {\n");
    body.push_str("    if (in_flight !== null) {\n");
    body.push_str("      in_flight.abort()\n");
    body.push_str("      in_flight = null\n");
    body.push_str("    }\n");
    body.push_str("  })\n\n");
    body.push_str("  return { data, error, loading, refetch }\n");
    body.push_str("}\n");
    body
}

fn render_create_composable(singular: &str) -> String {
    format!(
        "export function useCreate{singular}(): (input: {singular}Insertable) => Promise<{{ data?: {singular}Public; error?: MeltDownResponse }}> {{\n  return async (input: {singular}Insertable) => {{\n    const result \
         = await create{singular}(input)\n    if (result.error !== null) {{\n      return {{ error: result.error }}\n    }}\n    if (result.data === null) {{\n      return {{}}\n    }}\n    return {{ data: result.data \
         }}\n  }}\n}}\n",
        singular = singular,
    )
}

fn render_update_composable(singular: &str) -> String {
    format!(
        "export function useUpdate{singular}(): (id: number, patch: {singular}Patch) => Promise<{{ data?: {singular}Public; error?: MeltDownResponse }}> {{\n  return async (id: number, patch: {singular}Patch) => {{\n  \
           const result = await update{singular}(id, patch)\n    if (result.error !== null) {{\n      return {{ error: result.error }}\n    }}\n    if (result.data === null) {{\n      return {{}}\n    }}\n    return \
         {{ data: result.data }}\n  }}\n}}\n",
        singular = singular,
    )
}

fn render_delete_composable(singular: &str) -> String {
    format!(
        "export function useDelete{singular}(): (id: number) => Promise<{{ error?: MeltDownResponse }}> {{\n  return async (id: number) => {{\n    const result = await delete{singular}(id)\n    if (result.error !== \
         null) {{\n      return {{ error: result.error }}\n    }}\n    return {{}}\n  }}\n}}\n",
        singular = singular,
    )
}

fn plural_of_pascal(singular: &str, table: &str) -> String {
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use indexmap::IndexMap;

    use super::*;
    use crate::state::{
        names::{FieldName, ResourceName},
        resource::{AuthMode, FieldState, FieldVariant, ListOptions, PayloadShape, ResourceState, TopicScope, Verb, VerbState, WsEventsState, RESOURCE_SCHEMA_VERSION},
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

    fn synth_resource_with_ws() -> ResourceState {
        let mut r = synth_resource_all_verbs();
        r.ws_events = Some(WsEventsState {
            trigger_columns: BTreeSet::new(),
            payload_shape: PayloadShape::Public,
            topic_scope: TopicScope::Global,
        });
        r
    }

    #[test]
    fn exports_all_verb_composables() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("export function useUsersList("), "useUsersList missing");
        assert!(body.contains("export function useUser("), "useUser missing");
        assert!(body.contains("export function useCreateUser("), "useCreateUser missing");
        assert!(body.contains("export function useUpdateUser("), "useUpdateUser missing");
        assert!(body.contains("export function useDeleteUser("), "useDeleteUser missing");
    }

    #[test]
    fn imports_url_state_primitive() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("from '@/composables/url'"), "must import useUrlListState from hand-written primitive");
        assert!(!body.contains("from '@/generated/composables/url'"), "must NOT import url from generated");
    }

    #[test]
    fn imports_api_client() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("from '@/generated/api/users'"), "must import API client");
        assert!(body.contains("listUsers"));
        assert!(body.contains("getUser"));
        assert!(body.contains("createUser"));
        assert!(body.contains("updateUser"));
        assert!(body.contains("deleteUser"));
    }

    #[test]
    fn imports_types() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("from '@/generated/types/users'"), "must import types");
        assert!(body.contains("from '@/generated/types/meltdown'"), "must import meltdown");
    }

    #[test]
    fn list_uses_abort_controller() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("AbortController"), "must thread abort controller");
        assert!(body.contains(".abort()"), "must abort in-flight requests");
    }

    #[test]
    fn list_uses_url_state_not_local_refs() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("useUrlListState()"), "must call useUrlListState");
        assert!(body.contains("watch([page, pageSize, sort, filter]"), "must watch URL state writableComputedRefs");
    }

    #[test]
    fn list_exposes_total_and_total_pages() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("total: Ref<number>"), "must expose total");
        assert!(body.contains("total_pages: Ref<number>"), "must expose total_pages (snake_case)");
    }

    #[test]
    fn list_handles_visibility_change_for_polling() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("visibilitychange"), "must register visibilitychange listener");
        assert!(body.contains("document.visibilityState === 'hidden'"), "must check hidden state");
    }

    #[test]
    fn list_does_not_subscribe_when_no_ws_events() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(!body.contains("useChannel"), "no useChannel when no ws_events");
    }

    #[test]
    fn list_subscribes_when_ws_events_present() {
        let r = synth_resource_with_ws();
        let body = build_resource_composables(&r);
        assert!(body.contains("useChannel"), "must use useChannel when ws_events present");
        assert!(body.contains("'users.changed'"), "must subscribe to <table>.changed topic");
    }

    #[test]
    fn item_composable_takes_ref_id_and_watches_changes() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("export function useUser(id: Ref<number>)"));
        assert!(body.contains("watch(id"), "must watch id changes");
    }

    #[test]
    fn mutation_composables_return_async_fns() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(body.contains("UserInsertable) =>"), "create mutation takes Insertable");
        assert!(body.contains("UserPatch) =>"), "update mutation takes Patch");
    }

    #[test]
    fn no_optimistic_updates_in_mutations() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(!body.contains(".push("), "no array push in composables");
        assert!(!body.contains(".filter("), "no array filter in composables");
    }

    #[test]
    fn no_console_log() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(!body.contains("console.log"), "no console.log");
        assert!(!body.contains("console.warn"), "no console.warn");
        assert!(!body.contains("console.error"), "no console.error");
    }

    #[test]
    fn no_any_type() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(!body.contains(": any"), "no :any");
        assert!(!body.contains("as any"), "no as any");
        assert!(!body.contains("<any>"), "no <any>");
    }

    #[test]
    fn no_silent_fallbacks() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(!body.contains("?? []"), "fallback ?? bracket");
        assert!(!body.contains("?? {}"), "fallback ?? brace");
        assert!(!body.contains("?? 0"), "fallback ?? zero");
        assert!(!body.contains("|| []"), "fallback double-pipe bracket");
    }

    #[test]
    fn no_raw_fetch_call() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        for line in body.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("await fetch(") || trimmed.contains(" fetch(") || trimmed.contains("=fetch(") || trimmed.contains("(fetch(") {
                panic!("raw fetch found: {}", line);
            }
        }
        assert!(!body.contains("new WebSocket"), "no raw WS — must go through useChannel");
    }

    #[test]
    fn no_pinia_imports() {
        let r = synth_resource_all_verbs();
        let body = build_resource_composables(&r);
        assert!(!body.contains("from 'pinia'"));
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
