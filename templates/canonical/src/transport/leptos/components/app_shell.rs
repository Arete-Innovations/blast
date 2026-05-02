use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::{ButtonKind, RouteName};
use crate::transport::leptos::components::button::LinkButton;
use crate::transport::leptos::components::dark_mode_toggle::DarkModeToggle;
use crate::transport::leptos::components::generated::nav::AppNav;
use crate::transport::leptos::signals::session::use_session;

import_crate_style!(style, "src/transport/leptos/components/app_shell.module.scss");

#[component]
pub fn AppShell(title: String, children: Children) -> impl IntoView {
    let session = use_session();
    let has_user = move || session.get().is_some();
    let user_id = move || match session.get() {
        Some(ctx) => ctx.user_id.to_string(),
        None => "—".to_string(),
    };
    view! {
        <div class=style::shell>
            <aside class=style::sidebar>
                <div class=style::brand>
                    <span class=style::brand_kicker>"Catablast"</span>
                    <h1 class=style::brand_title>"App"</h1>
                </div>
                <AppNav/>
            </aside>
            <div class=style::main>
                <header class=style::topbar>
                    <h2 class=style::topbar_title>{title}</h2>
                    <div class=style::topbar_actions>
                        <Show when=has_user fallback=|| ()>
                            <span class=style::user_chip>"user #" {user_id.clone()}</span>
                        </Show>
                        <DarkModeToggle/>
                        <LinkButton href=RouteName::Logout.path().to_string() kind=ButtonKind::Ghost compact=true>
                            "Sign out"
                        </LinkButton>
                    </div>
                </header>
                <div class=style::content>
                    {children()}
                </div>
            </div>
        </div>
    }
}
