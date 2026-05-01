use leptos::prelude::*;
use stylance::import_crate_style;

use crate::transport::leptos::signals::url::use_query_dialog;

import_crate_style!(style, "src/transport/leptos/components/confirm_dialog.module.scss");

#[component]
pub fn ConfirmDialog(
    name: &'static str,
    title: String,
    message: String,
    confirm_label: String,
    on_confirm: Callback<()>,
) -> impl IntoView {
    let dialog = use_query_dialog(name);
    let visible = dialog.visible;
    let title_stored = StoredValue::new(title);
    let message_stored = StoredValue::new(message);
    let confirm_label_stored = StoredValue::new(confirm_label);

    let on_cancel = move |_ev: leptos::ev::MouseEvent| {
        dialog.close();
    };
    let on_overlay = move |_ev: leptos::ev::MouseEvent| {
        dialog.close();
    };
    let on_confirm_click = move |_ev: leptos::ev::MouseEvent| {
        on_confirm.run(());
        dialog.close();
    };

    view! {
        <Show when=move || visible.get() fallback=|| ()>
            <div class=style::overlay on:click=on_overlay></div>
            <div class=style::dialog role="dialog" aria-modal="true">
                <h2 class=style::title>{title_stored.get_value()}</h2>
                <p class=style::message>{message_stored.get_value()}</p>
                <div class=style::actions>
                    <button class=style::cancel on:click=on_cancel>"Cancel"</button>
                    <button class=style::confirm on:click=on_confirm_click>
                        {confirm_label_stored.get_value()}
                    </button>
                </div>
            </div>
        </Show>
    }
}
