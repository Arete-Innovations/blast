use crate::codegen::leptos_pages::shared::{
    auth_guard_mode_str, breadcrumb_inline_expr, cell_helpers_used, display_fields, formatter_calls, pretty_label,
};
use crate::state::resource::CustomLayout;
use crate::state::{AuthMode, FieldName, FieldState, ResourceState, Verb};

pub fn render_list_page(resource: &ResourceState, stem: &str, auth: AuthMode) -> String {
    let table = resource.name.as_str();
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}ListPage");
    let public_ty = format!("{stem}Public");
    let row_ty = format!("{stem}TableRow");
    let loader = format!("load_{table}_list");
    let label_pretty = pretty_label(stem);

    if let Some(layout) = resource.list_layout.as_ref() { // allow: optional Primer layout opt-in
        return render_list_page_custom(table, stem, &component, &public_ty, &loader, &label_pretty, auth_mode, layout);
    }

    let display = display_fields(resource);
    let has_create = match resource.verbs.get(&Verb::Create) {
        Some(state) => state.emit_html_page,
        None => false, // allow: absent Create verb means no "+ New" button
    };
    let helpers = cell_helpers_used(&display, false);
    let helpers_use = match helpers.is_empty() {
        true => String::new(),
        false => format!("use crate::views::components::cells::crud::{{{}}};\n", helpers.join(", ")),
    };
    let serde_value_use = match helpers.is_empty() {
        true => String::new(),
        false => "use serde_json::Value;\n".to_string(),
    };
    let is_public = matches!(auth, AuthMode::Public);
    let public_has_create = has_create && !is_public;

    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n\n");
    out.push_str(&serde_value_use);
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{{{public_ty}, {row_ty}}};\n"));
    out.push_str("use crate::structs::list_query::ListResponse;\n");
    let leptos_struct_imports = match (is_public, public_has_create) {
        (true, _) => "use crate::structs::vendored::leptos::{PageLayout, SkeletonVariant};\n",
        (false, _) => "use crate::structs::vendored::leptos::{BreadcrumbItem, ButtonKind, PageLayout, RouteName, SkeletonVariant};\n",
    };
    out.push_str(leptos_struct_imports);
    out.push_str("use crate::views::builders::TableBuilder;\n");
    out.push_str(&helpers_use);
    let component_imports = match is_public {
        true => "use crate::views::components::custom::PublicShell;\nuse crate::views::components::{topbar_auth_actions, AuthGuard, AuthGuardMode, Card, EmptyState, ErrorBanner, PageShell, Pagination, Skeleton};\n",
        false => "use crate::views::components::custom::DefaultAppShell;\nuse crate::views::components::{AuthGuard, AuthGuardMode, Breadcrumb, Card, EmptyState, ErrorBanner, LinkButton, PageShell, Pagination, Skeleton};\n",
    };
    out.push_str(component_imports);
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{loader};\n"));
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("use crate::views::signals::{use_topic_refetch, use_url_list_state};\n\n");

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component}() -> impl IntoView {{\n"));
    out.push_str(&format!(
        "    let items_signal: RwSignal<Option<::std::result::Result<ListResponse<{public_ty}>, MeltDown>>> = RwSignal::new(None);\n"
    ));
    out.push_str("    let refetch: RwSignal<u32> = RwSignal::new(0);\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("    use_topic_refetch(\"{table}:list\".to_string(), refetch);\n"));
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    let list_state = use_url_list_state();\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    Effect::new(move |_| {\n");
    out.push_str("        refetch.track();\n");
    out.push_str("        let query = list_state.to_list_query();\n");
    out.push_str("        leptos::task::spawn_local(async move {\n");
    out.push_str(&format!("            let result = {loader}(query).await;\n"));
    out.push_str("            items_signal.set(Some(result));\n");
    out.push_str("        });\n");
    out.push_str("    });\n\n");

    out.push_str("    view! {\n");
    out.push_str(&format!("        <AuthGuard mode={auth_mode}>\n"));
    out.push_str("            <PageShell layout=PageLayout::Bleed>\n");
    if is_public {
        out.push_str("                <PublicShell brand=crate::cfg().app.name.clone() topbar_actions=topbar_auth_actions()>\n");
        out.push_str("                    <div class=\"crud-toolbar\">\n");
        out.push_str("                        <div>\n");
        out.push_str(&format!("                            <h2 class=\"crud-toolbar__title\">\"{label_pretty}\"</h2>\n"));
        out.push_str(&format!(
            "                            <p class=\"crud-toolbar__subtitle\">\"All {label_lower} records\"</p>\n",
            label_lower = label_pretty.to_ascii_lowercase(),
        ));
        out.push_str("                        </div>\n");
        out.push_str("                    </div>\n");
        out.push_str("                    <Card>\n");
        out.push_str("                        {move || match items_signal.get() {\n");
        out.push_str("                            None => view! { <Skeleton variant=SkeletonVariant::Card/> }.into_any(),\n");
        out.push_str("                            Some(Ok(items)) => render_list_items(items).into_any(),\n");
        out.push_str("                            Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),\n");
        out.push_str("                        }}\n");
        out.push_str("                    </Card>\n");
        out.push_str("                </PublicShell>\n");
    } else {
        out.push_str(&format!("                <DefaultAppShell title=\"{label_pretty}\">\n"));
        out.push_str("                    <div class=\"crud-page__breadcrumb\">\n");
        out.push_str(&format!(
            "                        <Breadcrumb items={crumbs}/>\n",
            crumbs = breadcrumb_inline_expr(stem, &label_pretty, None),
        ));
        out.push_str("                    </div>\n");
        out.push_str("                    <div class=\"crud-toolbar\">\n");
        out.push_str("                        <div>\n");
        out.push_str(&format!("                            <h2 class=\"crud-toolbar__title\">\"{label_pretty}\"</h2>\n"));
        out.push_str(&format!(
            "                            <p class=\"crud-toolbar__subtitle\">\"All {label_lower} records\"</p>\n",
            label_lower = label_pretty.to_ascii_lowercase(),
        ));
        out.push_str("                        </div>\n");
        out.push_str("                        <div class=\"crud-toolbar__actions\">\n");
        if has_create {
            out.push_str(&format!(
                "                            <LinkButton href={{RouteName::ResourceCreate(\"{table}\").path().to_string()}} kind=ButtonKind::Primary>\"+ New {label_pretty}\"</LinkButton>\n"
            ));
        }
        out.push_str("                        </div>\n");
        out.push_str("                    </div>\n");
        out.push_str("                    <Card>\n");
        out.push_str("                        {move || match items_signal.get() {\n");
        out.push_str("                            None => view! { <Skeleton variant=SkeletonVariant::Card/> }.into_any(),\n");
        out.push_str("                            Some(Ok(items)) => render_list_items(items).into_any(),\n");
        out.push_str("                            Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),\n");
        out.push_str("                        }}\n");
        out.push_str("                    </Card>\n");
        out.push_str("                </DefaultAppShell>\n");
    }
    out.push_str("            </PageShell>\n");
    out.push_str("        </AuthGuard>\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(&render_list_helpers(table, stem, &public_ty, &row_ty, &display, public_has_create));
    out
}

fn render_list_helpers(table: &str, stem: &str, public_ty: &str, row_ty: &str, display: &[(&FieldName, &FieldState)], has_create: bool) -> String {
    let label_pretty = pretty_label(stem);
    let label_lower = label_pretty.to_ascii_lowercase();
    let mut out = String::new();
    out.push_str(&format!(
        "fn render_list_items(items: ListResponse<{public_ty}>) -> impl IntoView {{\n"
    ));
    out.push_str("    let total_pages = items.total_pages;\n");
    out.push_str("    let current_page = items.page as u64;\n");
    out.push_str(&format!(
        "    let rows: Vec<{row_ty}> = items.items.into_iter().map({row_ty}::from).collect();\n"
    ));
    out.push_str("    let has_rows = !rows.is_empty();\n");
    out.push_str("    let rows_stored = StoredValue::new(rows);\n");
    if has_create {
        out.push_str(&format!(
            "    let new_href = RouteName::ResourceCreate(\"{table}\").path().to_string();\n"
        ));
        out.push_str("    let new_href_stored = StoredValue::new(new_href);\n");
    }
    out.push_str("    view! {\n");
    out.push_str("        <Show\n");
    out.push_str("            when=move || has_rows\n");
    if has_create {
        out.push_str(&format!(
            "            fallback=move || view! {{ <div class=\"crud-empty\"><EmptyState title=\"No {label_lower} yet\".to_string() message=\"Click + New to add the first one.\".to_string()/><LinkButton href=new_href_stored.get_value() kind=ButtonKind::Primary>\"+ New {label_pretty}\"</LinkButton></div> }}.into_any()\n"
        ));
    } else {
        out.push_str(&format!(
            "            fallback=move || view! {{ <EmptyState title=\"No {label_lower} yet\".to_string() message=\"This list is empty.\".to_string()/> }}.into_any()\n"
        ));
    }
    out.push_str("        >\n");
    out.push_str("            {move || rows_stored.with_value(|rs| TableBuilder::new(rs.clone())\n");
    out.push_str(&formatter_calls(table, display, "                "));
    out.push_str(&format!("                .empty_text(\"No {label_lower}.\")\n"));
    out.push_str("                .into_view())}\n");
    out.push_str("        </Show>\n");
    out.push_str("        <Pagination total_pages current_page/>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

fn render_list_page_custom(
    table: &str,
    stem: &str,
    component: &str,
    public_ty: &str,
    loader: &str,
    label_pretty: &str,
    auth_mode: &str,
    layout: &CustomLayout,
) -> String {
    let cell_module = layout.module.as_str();
    let cell_component = layout.component.as_str();
    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n\n");
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{public_ty};\n"));
    out.push_str("use crate::structs::list_query::ListResponse;\n");
    out.push_str("use crate::structs::vendored::leptos::{BreadcrumbItem, PageLayout, RouteName, SkeletonVariant};\n");
    out.push_str(&format!(
        "use crate::views::components::vendored::{cell_module}::{cell_component};\n"
    ));
    out.push_str(
        "use crate::views::components::custom::DefaultAppShell;\nuse crate::views::components::{AuthGuard, AuthGuardMode, Breadcrumb, Card, EmptyState, ErrorBanner, PageShell, Skeleton};\n",
    );
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("use crate::transport::leptos::data::generated::{table}::{loader};\n"));
    out.push_str("#[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("use crate::views::signals::{use_topic_refetch, use_url_list_state};\n\n");

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component}() -> impl IntoView {{\n"));
    out.push_str(&format!(
        "    let items_signal: RwSignal<Option<::std::result::Result<ListResponse<{public_ty}>, MeltDown>>> = RwSignal::new(None);\n"
    ));
    out.push_str("    let refetch: RwSignal<u32> = RwSignal::new(0);\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str(&format!("    use_topic_refetch(\"{table}:list\".to_string(), refetch);\n"));
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    let list_state = use_url_list_state();\n");
    out.push_str("    #[cfg(target_arch = \"wasm32\")]\n");
    out.push_str("    Effect::new(move |_| {\n");
    out.push_str("        refetch.track();\n");
    out.push_str("        let query = list_state.to_list_query();\n");
    out.push_str("        leptos::task::spawn_local(async move {\n");
    out.push_str(&format!("            let result = {loader}(query).await;\n"));
    out.push_str("            items_signal.set(Some(result));\n");
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
    out.push_str("                    {move || match items_signal.get() {\n");
    out.push_str("                        None => view! { <Skeleton variant=SkeletonVariant::Card/> }.into_any(),\n");
    out.push_str(&format!(
        "                        Some(Ok(items)) => view! {{ <{cell_component} items=items.items/> }}.into_any(),\n"
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
