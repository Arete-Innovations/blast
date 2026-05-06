use crate::codegen::leptos_pages::shared::{auth_guard_mode_str, breadcrumb_inline_expr, pretty_label, render_id_signal_block};
use crate::state::AuthMode;

pub fn render_create_page(table: &str, stem: &str, auth: AuthMode) -> String {
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}CreatePage");
    let form_component = format!("{stem}CreateForm");
    let label_pretty = pretty_label(stem);

    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n\n");
    out.push_str("use crate::structs::vendored::leptos::{BreadcrumbItem, ButtonKind, PageLayout, RouteName};\n");
    out.push_str("use crate::views::components::{AppShell, AuthGuard, AuthGuardMode, Breadcrumb, Card, LinkButton, PageShell};\n");
    out.push_str(&format!("use crate::views::components::generated::forms::{table}::{form_component};\n\n"));

    out.push_str("#[component]\n");
    out.push_str(&format!("pub fn {component}() -> impl IntoView {{\n"));
    out.push_str("    view! {\n");
    out.push_str(&format!("        <AuthGuard mode={auth_mode}>\n"));
    out.push_str("            <PageShell layout=PageLayout::Bleed>\n");
    out.push_str(&format!("                <AppShell title=\"New {label_pretty}\".to_string()>\n"));
    out.push_str("                    <div class=\"crud-page__breadcrumb\">\n");
    out.push_str(&format!(
        "                        <Breadcrumb items={crumbs}/>\n",
        crumbs = breadcrumb_inline_expr(stem, "New", Some(&label_pretty)),
    ));
    out.push_str("                    </div>\n");
    out.push_str("                    <div class=\"crud-toolbar\">\n");
    out.push_str("                        <div>\n");
    out.push_str(&format!("                            <h2 class=\"crud-toolbar__title\">\"New {label_pretty}\"</h2>\n"));
    out.push_str(&format!(
        "                            <p class=\"crud-toolbar__subtitle\">\"Create a new {label_lower}\"</p>\n",
        label_lower = label_pretty.to_ascii_lowercase(),
    ));
    out.push_str("                        </div>\n");
    out.push_str("                        <div class=\"crud-toolbar__actions\">\n");
    out.push_str(&format!(
        "                            <LinkButton href={{RouteName::ResourceList(\"{table}\").path().to_string()}} kind=ButtonKind::Ghost>\"Cancel\"</LinkButton>\n"
    ));
    out.push_str("                        </div>\n");
    out.push_str("                    </div>\n");
    out.push_str("                    <Card title=\"Details\".to_string()>\n");
    out.push_str(&format!("                        <{form_component}/>\n"));
    out.push_str("                    </Card>\n");
    out.push_str("                </AppShell>\n");
    out.push_str("            </PageShell>\n");
    out.push_str("        </AuthGuard>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

pub fn render_edit_page(table: &str, stem: &str, auth: AuthMode) -> String {
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}EditPage");
    let form_component = format!("{stem}EditForm");
    let public_ty = format!("{stem}Public");
    let loader = format!("load_{table}_one");
    let label_pretty = pretty_label(stem);

    let mut out = String::new();
    out.push_str("use leptos::prelude::*;\n");
    out.push_str("use leptos::task::spawn_local;\n\n");
    out.push_str("use crate::meltdown::MeltDown;\n");
    out.push_str(&format!("use crate::structs::generated::{table}::{public_ty};\n"));
    out.push_str("use crate::structs::vendored::leptos::{BreadcrumbItem, ButtonKind, PageLayout, RouteName, SkeletonVariant};\n");
    out.push_str("use crate::views::components::{AppShell, AuthGuard, AuthGuardMode, Breadcrumb, Card, ErrorBanner, LinkButton, PageShell, Skeleton};\n");
    out.push_str(&format!("use crate::views::components::generated::forms::{table}::{form_component};\n"));
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
    out.push_str("    });\n\n");

    out.push_str(&format!(
        "    let detail_href_signal: Memo<String> = Memo::new(move |_| RouteName::ResourceDetail(\"{table}\", id_signal.get()).path().to_string());\n"
    ));
    out.push('\n');

    out.push_str("    view! {\n");
    out.push_str(&format!("        <AuthGuard mode={auth_mode}>\n"));
    out.push_str("            <PageShell layout=PageLayout::Bleed>\n");
    out.push_str(&format!("                <AppShell title=\"Edit {label_pretty}\".to_string()>\n"));
    out.push_str("                    <div class=\"crud-page__breadcrumb\">\n");
    out.push_str(&format!(
        "                        <Breadcrumb items={crumbs}/>\n",
        crumbs = breadcrumb_inline_expr(stem, "Edit", Some(&label_pretty)),
    ));
    out.push_str("                    </div>\n");
    out.push_str("                    <div class=\"crud-toolbar\">\n");
    out.push_str("                        <div>\n");
    out.push_str(&format!(
        "                            <h2 class=\"crud-toolbar__title\">{{move || format!(\"Edit {label_pretty} #{{}}\", id_signal.get())}}</h2>\n"
    ));
    out.push_str("                            <p class=\"crud-toolbar__subtitle\">\"Update fields and save\"</p>\n");
    out.push_str("                        </div>\n");
    out.push_str("                        <div class=\"crud-toolbar__actions\">\n");
    out.push_str("                            <LinkButton href=detail_href_signal.get() kind=ButtonKind::Ghost>\"Back\"</LinkButton>\n");
    out.push_str(&format!(
        "                            <LinkButton href={{RouteName::ResourceList(\"{table}\").path().to_string()}} kind=ButtonKind::Ghost>\"Cancel\"</LinkButton>\n"
    ));
    out.push_str("                        </div>\n");
    out.push_str("                    </div>\n");
    out.push_str("                    <Card title=\"Details\".to_string()>\n");
    out.push_str("                        {move || match item_signal.get() {\n");
    out.push_str("                            None => view! { <Skeleton variant=SkeletonVariant::Card/> }.into_any(),\n");
    out.push_str(&format!(
        "                            Some(Ok(initial)) => view! {{ <{form_component} initial=initial/> }}.into_any(),\n"
    ));
    out.push_str("                            Some(Err(err)) => view! { <ErrorBanner error=err/> }.into_any(),\n");
    out.push_str("                        }}\n");
    out.push_str("                    </Card>\n");
    out.push_str("                </AppShell>\n");
    out.push_str("            </PageShell>\n");
    out.push_str("        </AuthGuard>\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}
