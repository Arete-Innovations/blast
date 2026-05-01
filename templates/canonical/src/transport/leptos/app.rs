use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::transport::leptos::pages::{DashboardPage, LoginPage, NotFoundPage, ProfilePage, RegisterPage, WelcomePage};

pub fn shell(options: leptos::prelude::LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options=options.clone()/>
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

    view! {
        <Stylesheet id="leptos" href="/pkg/canonical.css"/>
        <Title text="Catablast"/>
        <Router>
            <Routes fallback=NotFoundPage>
                <Route path=path!("/") view=WelcomePage/>
                <Route path=path!("/login") view=LoginPage/>
                <Route path=path!("/register") view=RegisterPage/>
                <Route path=path!("/dashboard") view=DashboardPage/>
                <Route path=path!("/profile") view=ProfilePage/>
            </Routes>
        </Router>
    }
}
