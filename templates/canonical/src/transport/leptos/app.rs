use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::transport::leptos::components::ToastHost;
use crate::transport::leptos::data::auth::load_session;
use crate::transport::leptos::pages::{DashboardPage, LoginPage, NotFoundPage, ProfilePage, RegisterPage, WelcomePage};
use crate::transport::leptos::signals::session::provide_session_store;
use crate::transport::leptos::signals::toast::provide_toast_store;

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
    let session_store = provide_session_store();
    provide_toast_store();

    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match load_session().await {
                Ok(maybe) => session_store.set(maybe),
                Err(err) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    crate::cata_log!(Warning, format!("session load failed: {}", err));
                    #[cfg(target_arch = "wasm32")]
                    {
                        let msg: String = format!("session load failed: {}", err);
                        web_sys::console::warn_1(&msg.into());
                    }
                    session_store.set(None);
                }
            }
        });
    });

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
            <ToastHost/>
        </Router>
    }
}
