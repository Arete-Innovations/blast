use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::transport::leptos::components::ToastHost;
use crate::transport::leptos::pages::{DashboardPage, LoginPage, LogoutPage, NotFoundPage, ProfilePage, RegisterPage, WelcomePage};
use crate::transport::leptos::routes::GeneratedRoutes;
use crate::transport::leptos::signals::session::{provide_session_store, ssr_session_payload};
use crate::transport::leptos::signals::theme::{provide_theme_store, ssr_theme_str};
use crate::transport::leptos::signals::toast::provide_toast_store;

pub fn shell(options: leptos::prelude::LeptosOptions) -> impl IntoView {
    let css_href = format!("/pkg/{}.css", options.output_name.as_ref());
    let session_payload = ssr_session_payload();
    let theme_attr = ssr_theme_str();
    view! {
        <!DOCTYPE html>
        <html lang="en" data-theme=theme_attr>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <script id="cata-session-boot" inner_html=session_payload></script>
                <AutoReload options=options.clone() />
                <HydrationScripts options=options.clone()/>
                <link id="leptos" rel="stylesheet" href=css_href/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_session_store();
    provide_theme_store();
    provide_toast_store();

    view! {
        <Title text="Catablast"/>
        <Router>
            <Routes fallback=NotFoundPage>
                <Route path=path!("/") view=WelcomePage/>
                <Route path=path!("/login") view=LoginPage/>
                <Route path=path!("/logout") view=LogoutPage/>
                <Route path=path!("/register") view=RegisterPage/>
                <Route path=path!("/dashboard") view=DashboardPage/>
                <Route path=path!("/profile") view=ProfilePage/>
                <GeneratedRoutes/>
            </Routes>
            <ToastHost/>
        </Router>
    }
}
