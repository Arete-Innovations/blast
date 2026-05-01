use crate::state::AuthMode;

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
    let loader = format!("load_{table}_list");
    format!(
        "use leptos::prelude::*;\n\
         \n\
         use crate::meltdown::MeltDown;\n\
         use crate::structs::generated::{table}::{public_ty};\n\
         use crate::structs::list_query::ListResponse;\n\
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
         \x20                   Some(Ok(items)) => render_list_items(&items).into_any(),\n\
         \x20                   Some(Err(err)) => view! {{ <ErrorBanner error=err/> }}.into_any(),\n\
         \x20               }}}}\n\
         \x20           </PageShell>\n\
         \x20       </AuthGuard>\n\
         \x20   }}\n\
         }}\n\n\
         fn render_list_items(items: &ListResponse<{public_ty}>) -> impl IntoView {{\n\
         \x20   let rows: Vec<String> = items.items.iter().map(|row| format!(\"{{:?}}\", row)).collect();\n\
         \x20   view! {{\n\
         \x20       <ul>\n\
         \x20           {{rows.into_iter().map(|row| view! {{ <li>{{row}}</li> }}).collect_view()}}\n\
         \x20       </ul>\n\
         \x20   }}\n\
         }}\n",
    )
}

pub fn render_detail_page(table: &str, stem: &str, auth: AuthMode) -> String {
    let auth_mode = auth_guard_mode_str(&auth);
    let component = format!("{stem}DetailPage");
    let public_ty = format!("{stem}Public");
    let loader = format!("load_{table}_one");
    format!(
        "use leptos::prelude::*;\n\
         \n\
         use crate::meltdown::MeltDown;\n\
         use crate::structs::generated::{table}::{public_ty};\n\
         use crate::transport::leptos::components::{{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell}};\n\
         #[cfg(target_arch = \"wasm32\")]\n\
         use crate::transport::leptos::data::generated::{table}::{loader};\n\n\
         #[component]\n\
         pub fn {component}() -> impl IntoView {{\n\
         \x20   let item_signal: RwSignal<Option<::std::result::Result<{public_ty}, MeltDown>>> = RwSignal::new(None);\n\
         \x20   #[cfg(target_arch = \"wasm32\")]\n\
         \x20   Effect::new(move |_| {{\n\
         \x20       leptos::task::spawn_local(async move {{\n\
         \x20           let result = {loader}(0).await;\n\
         \x20           item_signal.set(Some(result));\n\
         \x20       }});\n\
         \x20   }});\n\
         \x20   view! {{\n\
         \x20       <AuthGuard mode={auth_mode}>\n\
         \x20           <PageShell layout=PageLayout::Cards>\n\
         \x20               <h1>\"{stem} detail\"</h1>\n\
         \x20               {{move || match item_signal.get() {{\n\
         \x20                   None => view! {{ <p>\"Loading...\"</p> }}.into_any(),\n\
         \x20                   Some(Ok(item)) => render_detail_item(&item).into_any(),\n\
         \x20                   Some(Err(err)) => view! {{ <ErrorBanner error=err/> }}.into_any(),\n\
         \x20               }}}}\n\
         \x20           </PageShell>\n\
         \x20       </AuthGuard>\n\
         \x20   }}\n\
         }}\n\n\
         fn render_detail_item(item: &{public_ty}) -> impl IntoView {{\n\
         \x20   let body = format!(\"{{:?}}\", item);\n\
         \x20   view! {{ <pre>{{body}}</pre> }}\n\
         }}\n",
    )
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
    format!(
        "use leptos::prelude::*;\n\
         \n\
         use crate::meltdown::MeltDown;\n\
         use crate::structs::generated::{table}::{public_ty};\n\
         use crate::transport::leptos::components::{{AuthGuard, AuthGuardMode, ErrorBanner, PageLayout, PageShell}};\n\
         use crate::transport::leptos::components::generated::forms::{table}::{form_component};\n\
         #[cfg(target_arch = \"wasm32\")]\n\
         use crate::transport::leptos::data::generated::{table}::{loader};\n\n\
         #[component]\n\
         pub fn {component}() -> impl IntoView {{\n\
         \x20   let item_signal: RwSignal<Option<::std::result::Result<{public_ty}, MeltDown>>> = RwSignal::new(None);\n\
         \x20   #[cfg(target_arch = \"wasm32\")]\n\
         \x20   Effect::new(move |_| {{\n\
         \x20       leptos::task::spawn_local(async move {{\n\
         \x20           let result = {loader}(0).await;\n\
         \x20           item_signal.set(Some(result));\n\
         \x20       }});\n\
         \x20   }});\n\
         \x20   view! {{\n\
         \x20       <AuthGuard mode={auth_mode}>\n\
         \x20           <PageShell layout=PageLayout::Cards>\n\
         \x20               <h1>\"Edit {stem}\"</h1>\n\
         \x20               {{move || match item_signal.get() {{\n\
         \x20                   None => view! {{ <p>\"Loading...\"</p> }}.into_any(),\n\
         \x20                   Some(Ok(initial)) => view! {{ <{form_component} initial=initial/> }}.into_any(),\n\
         \x20                   Some(Err(err)) => view! {{ <ErrorBanner error=err/> }}.into_any(),\n\
         \x20               }}}}\n\
         \x20           </PageShell>\n\
         \x20       </AuthGuard>\n\
         \x20   }}\n\
         }}\n",
    )
}
