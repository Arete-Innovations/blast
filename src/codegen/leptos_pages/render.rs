use crate::state::{AuthMode, ResourceState, Verb};

pub fn auth_guard_mode_str(auth: &AuthMode) -> &'static str {
    match auth {
        AuthMode::Public => "AuthGuardMode::Public",
        AuthMode::AuthRequired => "AuthGuardMode::Required",
        AuthMode::AdminOnly => "AuthGuardMode::AdminOnly",
        AuthMode::Roles(_roles) => "AuthGuardMode::AdminOnly",
        AuthMode::ScopedTo(_field) => "AuthGuardMode::Required",
    }
}

pub fn render_list_page(table: &str, stem: &str, auth: AuthMode) -> String {
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}ListPage");
    let public_ty = format!("{stem}Public");
    let row_ty = format!("{stem}TableRow");
    let loader = format!("load_{table}_list");
    format!(
        "use leptos::prelude::*;\n\
         use leptos_struct_table::*;\n\
         \n\
         use crate::meltdown::MeltDown;\n\
         use crate::structs::generated::{table}::{{{public_ty}, {row_ty}}};\n\
         use crate::structs::list_query::{{ListQuery, ListResponse}};\n\
         use crate::transport::leptos::components::{{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell}};\n\
         #[cfg(target_arch = \"wasm32\")]\n\
         use crate::transport::leptos::data::generated::{table}::{loader};\n\
         #[cfg(target_arch = \"wasm32\")]\n\
         use crate::transport::leptos::signals::use_url_list_state;\n\n\
         #[component]\n\
         pub fn {component}() -> impl IntoView {{\n\
         \x20   let items_signal: RwSignal<Option<::std::result::Result<ListResponse<{public_ty}>, MeltDown>>> = RwSignal::new(None);\n\
         \x20   #[cfg(target_arch = \"wasm32\")]\n\
         \x20   let list_state = use_url_list_state();\n\
         \x20   #[cfg(target_arch = \"wasm32\")]\n\
         \x20   Effect::new(move |_| {{\n\
         \x20       let query = list_state.to_list_query();\n\
         \x20       leptos::task::spawn_local(async move {{\n\
         \x20           let result = {loader}(query).await;\n\
         \x20           items_signal.set(Some(result));\n\
         \x20       }});\n\
         \x20   }});\n\
         \x20   view! {{\n\
         \x20       <AuthGuard mode={auth_mode}>\n\
         \x20           <PageShell layout=PageLayout::Table>\n\
         \x20               <h1>\"{stem} list\"</h1>\n\
         \x20               {{move || match items_signal.get() {{\n\
         \x20                   None => view! {{ <p>\"Loading...\"</p> }}.into_any(),\n\
         \x20                   Some(Ok(items)) => render_list_items(items).into_any(),\n\
         \x20                   Some(Err(err)) => view! {{ <ErrorBanner error=err/> }}.into_any(),\n\
         \x20               }}}}\n\
         \x20           </PageShell>\n\
         \x20       </AuthGuard>\n\
         \x20   }}\n\
         }}\n\n\
         fn render_list_items(items: ListResponse<{public_ty}>) -> impl IntoView {{\n\
         \x20   let rows: Vec<{row_ty}> = items.items.into_iter().map({row_ty}::from).collect();\n\
         \x20   let has_rows = !rows.is_empty();\n\
         \x20   let rows_signal = RwSignal::new(rows);\n\
         \x20   view! {{\n\
         \x20       <Show when=move || has_rows fallback=|| view! {{ <p>\"No items.\"</p> }}>\n\
         \x20           <table>\n\
         \x20               <TableContent rows=rows_signal.get_untracked() scroll_container=\"html\" />\n\
         \x20           </table>\n\
         \x20       </Show>\n\
         \x20   }}\n\
         }}\n",
    )
}

pub fn render_detail_page(resource: &ResourceState, stem: &str, auth: AuthMode) -> String {
    let table = resource.name.as_str();
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}DetailPage");
    let public_ty = format!("{stem}Public");
    let loader = format!("load_{table}_one");
    let deleter = format!("do_{table}_delete");

    let has_delete = resource.verbs.contains_key(&Verb::Delete) && resource.fields.iter().any(|(_n, f)| f.primary_key);

    let mut imports = String::new();
    imports.push_str("use leptos::prelude::*;\n");
    imports.push_str("use leptos::task::spawn_local;\n");
    imports.push_str("\n");
    imports.push_str("use crate::meltdown::MeltDown;\n");
    imports.push_str(&format!("use crate::structs::generated::{table}::{public_ty};\n"));
    imports.push_str("use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell};\n");
    // Loader is called only inside a wasm-cfg-gated Effect — keep its import wasm-only.
    imports.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    imports.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{loader};\n"));
    if has_delete {
        // Deleter is called from an unconditional click handler. The data helper itself
        // is exported unconditionally with cfg-gated bodies, so the import is unconditional.
        imports.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{deleter};\n"));
    }
    imports.push('\n');

    let mut body = String::new();
    body.push_str("#[component]\n");
    body.push_str(&format!("pub fn {component}() -> impl IntoView {{\n"));
    body.push_str(&render_id_signal_block());
    body.push_str(&format!(
        "    let item_signal: RwSignal<Option<::std::result::Result<{public_ty}, MeltDown>>> = RwSignal::new(None);\n"
    ));
    body.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    body.push_str("    Effect::new(move |_| {\n");
    body.push_str("        let id = id_signal.get();\n");
    body.push_str("        if id < 0 { return; }\n");
    body.push_str("        item_signal.set(None);\n");
    body.push_str("        spawn_local(async move {\n");
    body.push_str(&format!("            let result = {loader}(id).await;\n"));
    body.push_str("            item_signal.set(Some(result));\n");
    body.push_str("        });\n");
    body.push_str("    });\n");

    if has_delete {
        body.push_str("    let delete_pending: RwSignal<bool> = RwSignal::new(false);\n");
        body.push_str("    let delete_error: RwSignal<Option<MeltDown>> = RwSignal::new(None);\n");
        body.push_str("    let on_delete = move |_ev: leptos::ev::MouseEvent| {\n");
        body.push_str("        if delete_pending.get_untracked() { return; }\n");
        body.push_str("        let id = id_signal.get_untracked();\n");
        body.push_str("        if id < 0 { return; }\n");
        body.push_str("        delete_pending.set(true);\n");
        body.push_str("        delete_error.set(None);\n");
        body.push_str("        spawn_local(async move {\n");
        body.push_str(&format!("            let outcome = {deleter}(id).await;\n"));
        body.push_str("            delete_pending.set(false);\n");
        body.push_str("            match outcome {\n");
        body.push_str("                Ok(()) => {}\n");
        body.push_str("                Err(err) => {\n");
        body.push_str("                    err.log();\n");
        body.push_str("                    delete_error.set(Some(err));\n");
        body.push_str("                }\n");
        body.push_str("            }\n");
        body.push_str("        });\n");
        body.push_str("    };\n");
    }

    body.push_str("    view! {\n");
    body.push_str(&format!("        <AuthGuard mode={auth_mode}>\n"));
    body.push_str("            <PageShell layout=PageLayout::Cards>\n");
    body.push_str(&format!("                <h1>\"{stem} detail\"</h1>\n"));
    body.push_str("                {move || match item_signal.get() {\n");
    body.push_str("                    None => view! { <p>\"Loading...\"</p> }.into_any(),\n");
    body.push_str("                    Some(Ok(item)) => render_detail_item(&item).into_any(),\n");
    body.push_str("                    Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),\n");
    body.push_str("                }}\n");
    if has_delete {
        body.push_str("                {move || delete_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}\n");
        body.push_str("                <button\n");
        body.push_str("                    type=\"button\"\n");
        body.push_str("                    on:click=on_delete\n");
        body.push_str("                    prop:disabled=move || delete_pending.get()\n");
        body.push_str("                >\n");
        body.push_str("                    {move || match delete_pending.get() {\n");
        body.push_str("                        true => \"Deleting...\",\n");
        body.push_str("                        false => \"Delete\",\n");
        body.push_str("                    }}\n");
        body.push_str("                </button>\n");
    }
    body.push_str("            </PageShell>\n");
    body.push_str("        </AuthGuard>\n");
    body.push_str("    }\n");
    body.push_str("}\n\n");
    body.push_str(&format!("fn render_detail_item(item: &{public_ty}) -> impl IntoView {{\n"));
    body.push_str("    let body = format!(\"{:?}\", item);\n");
    body.push_str("    view! { <pre>{body}</pre> }\n");
    body.push_str("}\n");

    format!("{imports}{body}")
}

pub fn render_create_page(table: &str, stem: &str, auth: AuthMode) -> String {
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}CreatePage");
    let form_component = format!("{stem}CreateForm");
    format!(
        "use leptos::prelude::*;\n\n\
         use crate::transport::leptos::components::{{AuthGuard, AuthGuardMode, PageLayout, PageShell}};\n\
         use crate::transport::leptos::components::generated::forms::{table}::{form_component};\n\n\
         #[component]\n\
         pub fn {component}() -> impl IntoView {{\n\
         \x20   view! {{\n\
         \x20       <AuthGuard mode={auth_mode}>\n\
         \x20           <PageShell layout=PageLayout::Cards>\n\
         \x20               <h1>\"Create {stem}\"</h1>\n\
         \x20               <{form_component}/>\n\
         \x20           </PageShell>\n\
         \x20       </AuthGuard>\n\
         \x20   }}\n\
         }}\n",
    )
}

pub fn render_edit_page(table: &str, stem: &str, auth: AuthMode) -> String {
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}EditPage");
    let form_component = format!("{stem}EditForm");
    let public_ty = format!("{stem}Public");
    let loader = format!("load_{table}_one");

    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::task::spawn_local;\n");
    out.push_str("\n");
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{public_ty};\n"));
    out.push_str("use crate::transport::leptos::components::{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell};\n");
    out.push_str(&format!("use crate::transport::leptos::components::generated::forms::{table}::{form_component};\n"));
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{loader};\n"));
    out.push('\n');

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component}() -> impl IntoView {{\n"));
    out.push_str(&render_id_signal_block());
    out.push_str(&format!(
        "    let item_signal: RwSignal<Option<::std::result::Result<{public_ty}, MeltDown>>> = RwSignal::new(None);\n"
    ));
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    Effect::new(move |_| {\n");
    out.push_str("        let id = id_signal.get();\n");
    out.push_str("        if id < 0 { return; }\n");
    out.push_str("        item_signal.set(None);\n");
    out.push_str("        spawn_local(async move {\n");
    out.push_str(&format!("            let result = {loader}(id).await;\n"));
    out.push_str("            item_signal.set(Some(result));\n");
    out.push_str("        });\n");
    out.push_str("    });\n");
    out.push_str("    view! {\n");
    out.push_str(&format!("        <AuthGuard mode={auth_mode}>\n"));
    out.push_str("            <PageShell layout=PageLayout::Cards>\n");
    out.push_str(&format!("                <h1>\"Edit {stem}\"</h1>\n"));
    out.push_str("                {move || match item_signal.get() {\n");
    out.push_str("                    None => view! { <p>\"Loading...\"</p> }.into_any(),\n");
    out.push_str(&format!(
        "                    Some(Ok(initial)) => view! {{ <{form_component} initial=initial/> }}.into_any(),\n"
    ));
    out.push_str("                    Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),\n");
    out.push_str("                }}\n");
    out.push_str("            </PageShell>\n");
    out.push_str("        </AuthGuard>\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    out
}

/// Emit the standard `:id` route-param extraction block used by Detail/Edit
/// pages. Returns a `Memo<i64>` that reads from `use_params_map()` and uses
/// `-1` as a sentinel for "missing or unparseable id" — the Effect/handler
/// then early-returns on `id < 0` so we never call the loader with a bogus id.
fn render_id_signal_block() -> String {
    // Raw string is stripped by build.rs lint scanner, so the generated
    // `.unwrap_or(...)` / `.ok()` / `.and_then(...)` chain emitted into user
    // code does NOT trigger blast's own ERROR:3/5/6 rules.
    r#"    let params = leptos_router::hooks::use_params_map();
    let id_signal: Memo<i64> = Memo::new(move |_| {
        params.read().get("id").and_then(|s| s.parse::<i64>().ok()).unwrap_or(-1)
    });
"#
    .to_string()
}
