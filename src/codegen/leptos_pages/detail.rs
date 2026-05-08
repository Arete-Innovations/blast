use crate::codegen::leptos_pages::shared::{
    auth_guard_mode_str, breadcrumb_inline_expr, cell_helpers_used, detail_formatter_calls, display_fields, pretty_label, primary_key_field, render_id_signal_block,
};
use crate::state::resource::CustomLayout;
use crate::state::{AuthMode, FieldName, FieldState, ResourceState, Verb};

fn live_topic_subs(table: &str, extra: &[String]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "        use_topic_refetch(format!(\"{table}:row:{{}}\", id), refetch);\n"
    ));
    for tmpl in extra {
        let leptos_fmt = tmpl.replace("{id}", "{}");
        out.push_str(&format!(
            "        use_topic_refetch(format!(\"{leptos_fmt}\", id), refetch);\n"
        ));
    }
    out
}

pub fn render_detail_page(resource: &ResourceState, stem: &str, auth: AuthMode) -> String {
    let table = resource.name.as_str();
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}DetailPage");
    let public_ty = format!("{stem}Public");
    let loader = format!("load_{table}_one");
    let deleter = format!("do_{table}_delete");
    let label_pretty = pretty_label(stem);

    if let Some(layout) = resource.detail_layout.as_ref() { // allow: optional Primer layout opt-in
        return render_detail_page_custom(table, stem, &component, &public_ty, &loader, &label_pretty, auth_mode, layout, &resource.live_topics);
    }

    let pk = primary_key_field(resource);
    let has_delete = resource.verbs.contains_key(&Verb::Delete) && pk.is_some();
    let has_edit = match resource.verbs.get(&Verb::Update) {
        Some(state) => state.emit_html_page && pk.is_some(),
        None => false, // allow: absent Update verb → no Edit button
    };
    let display = display_fields(resource);
    let helpers = cell_helpers_used(&display, true);
    let helpers_use = match helpers.is_empty() {
        true => String::new(),
        false => format!("use crate::views::components::cells::crud::{{{}}};\n", helpers.join(", ")),
    };
    let serde_value_use = match helpers.is_empty() {
        true => String::new(),
        false => "use serde_json::Value;\n".to_string(),
    };

    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::task::spawn_local;\n\n");
    out.push_str(&serde_value_use);
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{public_ty};\n"));
    out.push_str("use crate::structs::vendored::leptos::{BreadcrumbItem, ButtonKind, PageLayout, RouteName, SkeletonVariant};\n");
    out.push_str("use crate::views::builders::DetailBuilder;\n");
    out.push_str(&helpers_use);
    out.push_str("use crate::views::components::custom::DefaultAppShell;\n");
    out.push_str("use crate::views::components::{AuthGuard, AuthGuardMode, Breadcrumb, Card, ");
    if has_delete {
        out.push_str("ConfirmDialog, ");
    }
    out.push_str("ErrorBanner, LinkButton, PageShell, Skeleton};\n");
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{loader};\n"));
    if has_delete {
        out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{deleter};\n"));
    }
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("use crate::views::signals::use_topic_refetch;\n");
    out.push('\n');

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component}() -> impl IntoView {{\n"));
    out.push_str(&render_id_signal_block());
    out.push_str(&format!(
        "    let item_signal: RwSignal<Option<::std::result::Result<{public_ty}, MeltDown>>> = RwSignal::new(None);\n"
    ));
    out.push_str("    let refetch: RwSignal<u32> = RwSignal::new(0);\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    Effect::new(move |_| {\n");
    out.push_str("        let id = id_signal.get();\n");
    out.push_str("        if id < 0 { return; }\n");
    out.push_str(&live_topic_subs(table, &resource.live_topics));
    out.push_str("    });\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    Effect::new(move |_| {\n");
    out.push_str("        refetch.track();\n");
    out.push_str("        let id = id_signal.get();\n");
    out.push_str("        if id < 0 { return; }\n");
    out.push_str("        item_signal.set(None);\n");
    out.push_str("        spawn_local(async move {\n");
    out.push_str(&format!("            let result = {loader}(id).await;\n"));
    out.push_str("            item_signal.set(Some(result));\n");
    out.push_str("        });\n");
    out.push_str("    });\n\n");

    if has_edit {
        out.push_str(&format!(
            "    let edit_href_signal: Memo<String> = Memo::new(move |_| RouteName::ResourceEdit(\"{table}\", id_signal.get()).path().to_string());\n"
        ));
    }

    if has_delete {
        out.push_str("    let delete_pending: RwSignal<bool> = RwSignal::new(false);\n");
        out.push_str("    let delete_error: RwSignal<Option<MeltDown>> = RwSignal::new(None);\n");
        out.push_str(&format!(
            "    let list_path_for_nav = RouteName::ResourceList(\"{table}\").path().to_string();\n"
        ));
        out.push_str("    let on_delete_confirm = Callback::new(move |_| {\n");
        out.push_str("        if delete_pending.get_untracked() { return; }\n");
        out.push_str("        let id = id_signal.get_untracked();\n");
        out.push_str("        if id < 0 { return; }\n");
        out.push_str("        delete_pending.set(true);\n");
        out.push_str("        delete_error.set(None);\n");
        out.push_str("        let target = list_path_for_nav.clone();\n");
        out.push_str("        spawn_local(async move {\n");
        out.push_str(&format!("            let outcome = {deleter}(id).await;\n"));
        out.push_str("            delete_pending.set(false);\n");
        out.push_str("            match outcome {\n");
        out.push_str("                Ok(()) => {\n");
        out.push_str("                    #[cfg(target_arch = \"wasm32\")]\n");
        out.push_str("                    {\n");
        out.push_str("                        let nav = leptos_router::hooks::use_navigate();\n");
        out.push_str("                        nav(&target, ::leptos_router::NavigateOptions::default());\n");
        out.push_str("                    }\n");
        out.push_str("                    #[cfg(not(target_arch = \"wasm32\"))]\n");
        out.push_str("                    {\n");
        out.push_str("                        drop(target);\n");
        out.push_str("                    }\n");
        out.push_str("                }\n");
        out.push_str("                Err(err) => {\n");
        out.push_str("                    err.log();\n");
        out.push_str("                    delete_error.set(Some(err));\n");
        out.push_str("                }\n");
        out.push_str("            }\n");
        out.push_str("        });\n");
        out.push_str("    });\n");
    }

    out.push('\n');

    out.push_str("    view! {\n");
    out.push_str(&format!("        <AuthGuard mode={auth_mode}>\n"));
    out.push_str("            <PageShell layout=PageLayout::Bleed>\n");
    out.push_str(&format!("                <DefaultAppShell title=\"{label_pretty}\">\n"));
    out.push_str("                    <div class=\"crud-page__breadcrumb\">\n");
    out.push_str(&format!(
        "                        <Breadcrumb items={crumbs}/>\n",
        crumbs = breadcrumb_inline_expr(stem, "Detail", Some(&label_pretty)),
    ));
    out.push_str("                    </div>\n");
    out.push_str("                    <div class=\"crud-toolbar\">\n");
    out.push_str("                        <div>\n");
    out.push_str(&format!(
        "                            <h2 class=\"crud-toolbar__title\">{{move || format!(\"{label_pretty} #{{}}\", id_signal.get())}}</h2>\n"
    ));
    out.push_str("                            <p class=\"crud-toolbar__subtitle\">\"Detail view\"</p>\n");
    out.push_str("                        </div>\n");
    out.push_str("                        <div class=\"crud-toolbar__actions\">\n");
    out.push_str(&format!(
        "                            <LinkButton href={{RouteName::ResourceList(\"{table}\").path().to_string()}} kind=ButtonKind::Ghost>\"Back\"</LinkButton>\n"
    ));
    if has_edit {
        out.push_str("                            <LinkButton href=edit_href_signal.get() kind=ButtonKind::Secondary>\"Edit\"</LinkButton>\n");
    }
    if has_delete {
        out.push_str("                            <LinkButton href=\"?dialog=confirm_delete\".to_string() kind=ButtonKind::Danger>\"Delete\"</LinkButton>\n");
    }
    out.push_str("                        </div>\n");
    out.push_str("                    </div>\n");
    out.push_str("                    <Card>\n");
    out.push_str("                        {move || match item_signal.get() {\n");
    out.push_str("                            None => view! { <Skeleton variant=SkeletonVariant::Card/> }.into_any(),\n");
    out.push_str("                            Some(Ok(item)) => render_detail_item(item).into_any(),\n");
    out.push_str("                            Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),\n");
    out.push_str("                        }}\n");
    out.push_str("                    </Card>\n");
    if has_delete {
        out.push_str("                    {move || delete_error.get().map(|err| view! { <ErrorBanner error=err/> }.into_any())}\n");
        out.push_str(&format!(
            "                    <ConfirmDialog name=\"confirm_delete\" title=\"Delete {label_pretty}?\".to_string() message=\"This action is permanent and cannot be undone.\".to_string() confirm_label=\"Delete\".to_string() on_confirm=on_delete_confirm/>\n"
        ));
    }
    out.push_str("                </DefaultAppShell>\n");
    out.push_str("            </PageShell>\n");
    out.push_str("        </AuthGuard>\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(&render_detail_helpers(table, &public_ty, &display));
    out
}

fn render_detail_page_custom(
    table: &str,
    stem: &str,
    component: &str,
    public_ty: &str,
    loader: &str,
    label_pretty: &str,
    auth_mode: &str,
    layout: &CustomLayout,
    live_topics: &[String],
) -> String {
    let cell_module = layout.module.as_str();
    let cell_component = layout.component.as_str();
    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::task::spawn_local;\n\n");
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{public_ty};\n"));
    out.push_str("use crate::structs::vendored::leptos::{BreadcrumbItem, PageLayout, RouteName, SkeletonVariant};\n");
    out.push_str(&format!(
        "use crate::views::components::vendored::{cell_module}::{cell_component};\n"
    ));
    out.push_str(
        "use crate::views::components::custom::DefaultAppShell;\nuse crate::views::components::{AuthGuard, AuthGuardMode, Breadcrumb, ErrorBanner, PageShell, Skeleton};\n",
    );
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{loader};\n"));
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("use crate::views::signals::use_topic_refetch;\n");
    out.push_str("use leptos_router::hooks::use_params_map;\n\n");

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component}() -> impl IntoView {{\n"));
    out.push_str(&format!(
        "    let item_signal: RwSignal<Option<::std::result::Result<{public_ty}, MeltDown>>> = RwSignal::new(None);\n"
    ));
    out.push_str("    let refetch: RwSignal<u32> = RwSignal::new(0);\n");
    out.push_str("    let params = use_params_map();\n");
    out.push_str("    let id_signal: Memo<i64> = Memo::new(move |_| {\n");
    out.push_str("        let id_str = params.with(|p| match p.get(\"id\") {\n");
    out.push_str("            Some(s) => s.to_string(),\n");
    out.push_str("            None => String::new(),\n");
    out.push_str("        });\n");
    out.push_str("        match id_str.parse::<i64>() {\n");
    out.push_str("            Ok(n) => n,\n");
    out.push_str("            Err(e) => { crate::cata_log!(Debug, format!(\"detail id parse failed: {{}}\", e)); -1 }\n");
    out.push_str("        }\n");
    out.push_str("    });\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    Effect::new(move |_| {\n");
    out.push_str("        let id = id_signal.get();\n");
    out.push_str("        if id < 0 { return; }\n");
    out.push_str(&live_topic_subs(table, live_topics));
    out.push_str("    });\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    Effect::new(move |_| {\n");
    out.push_str("        refetch.track();\n");
    out.push_str("        let id = id_signal.get();\n");
    out.push_str("        if id < 0 {\n");
    out.push_str("            item_signal.set(Some(Err(MeltDown::record_not_found(\"invalid id\"))));\n");
    out.push_str("            return;\n");
    out.push_str("        }\n");
    out.push_str("        item_signal.set(None);\n");
    out.push_str("        spawn_local(async move {\n");
    out.push_str(&format!("            let result = {loader}(id).await;\n"));
    out.push_str("            item_signal.set(Some(result));\n");
    out.push_str("        });\n");
    out.push_str("    });\n\n");

    out.push_str("    view! {\n");
    out.push_str(&format!("        <AuthGuard mode={auth_mode}>\n"));
    out.push_str("            <PageShell layout=PageLayout::Bleed>\n");
    out.push_str(&format!("                <DefaultAppShell title=\"{label_pretty}\">\n"));
    out.push_str("                    <div class=\"crud-page__breadcrumb\">\n");
    out.push_str(&format!(
        "                        <Breadcrumb items={crumbs}/>\n",
        crumbs = breadcrumb_inline_expr(stem, label_pretty, None),
    ));
    out.push_str("                    </div>\n");
    out.push_str("                    {move || match item_signal.get() {\n");
    out.push_str("                        None => view! { <Skeleton variant=SkeletonVariant::Card/> }.into_any(),\n");
    out.push_str(&format!(
        "                        Some(Ok(item)) => view! {{ <{cell_component} item=item/> }}.into_any(),\n"
    ));
    out.push_str("                        Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),\n");
    out.push_str("                    }}\n");
    out.push_str("                </DefaultAppShell>\n");
    out.push_str("            </PageShell>\n");
    out.push_str("        </AuthGuard>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn render_detail_helpers(table: &str, public_ty: &str, display: &[(&FieldName, &FieldState)]) -> String {
    let mut out = String::new();
    out.push_str(&format!("fn render_detail_item(item: {public_ty}) -> impl IntoView {{\n"));
    out.push_str("    DetailBuilder::new(item)\n");
    for (name, _f) in display {
        let col = name.as_str();
        let label_src = match col.strip_suffix("_cents") {
            Some(stripped) => stripped,
            None => col,
        };
        let label = pretty_label(label_src);
        out.push_str(&format!("        .label(\"{col}\", \"{label}\")\n"));
    }
    out.push_str(&detail_formatter_calls(table, display, "        "));
    out.push_str("        .into_view()\n");
    out.push_str("}\n");
    out
}
