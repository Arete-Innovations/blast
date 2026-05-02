use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/auth_card.module.scss");

#[component]
pub fn AuthCard(
    title: String,
    #[prop(default = None)] lede: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=style::wrap>
            <div class=style::card>
                <div class=style::brand>
                    <span class=style::brand_kicker>"Catablast"</span>
                    <h1 class=style::title>{title}</h1>
                    {lede.map(|t| view! { <p class=style::lede>{t}</p> })}
                </div>
                {children()}
            </div>
        </div>
    }
}

#[component]
pub fn AuthCardAlt(children: Children) -> impl IntoView {
    view! {
        <p class=style::alt>{children()}</p>
    }
}
