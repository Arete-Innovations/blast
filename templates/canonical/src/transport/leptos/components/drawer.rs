use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::DrawerSide;
use crate::transport::leptos::signals::url::use_query_dialog;

import_crate_style!(style, "src/transport/leptos/components/drawer.module.scss");

#[component]
pub fn Drawer(
    name: &'static str,
    side: DrawerSide,
    title: String,
    children: ChildrenFn,
) -> impl IntoView {
    let dialog = use_query_dialog(name);
    let visible = dialog.visible;
    let children_stored = StoredValue::new(children);
    let title_stored = StoredValue::new(title);

    let on_overlay = move |_ev: leptos::ev::MouseEvent| {
        dialog.close();
    };
    let on_close = move |_ev: leptos::ev::MouseEvent| {
        dialog.close();
    };

    view! {
        <Show when=move || visible.get() fallback=|| ()>
            <div class=style::overlay on:click=on_overlay></div>
            <aside
                class=style::drawer
                data-side=side.as_str()
                role="dialog"
                aria-modal="true"
            >
                <header class=style::header>
                    <h2 class=style::title>{title_stored.get_value()}</h2>
                    <button class=style::close on:click=on_close aria-label="Close">"\u{00d7}"</button>
                </header>
                <div class=style::body>
                    {children_stored.with_value(|c| c())}
                </div>
            </aside>
        </Show>
    }
}
