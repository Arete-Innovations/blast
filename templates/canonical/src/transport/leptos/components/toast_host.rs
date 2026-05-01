use leptos::prelude::*;
use stylance::import_crate_style;

use crate::transport::leptos::signals::toast::{use_toast, ToastKind};

import_crate_style!(style, "src/transport/leptos/components/toast_host.module.scss");

#[component]
pub fn ToastHost() -> impl IntoView {
    let toasts = use_toast();
    let list = toasts.list();

    view! {
        <div class=style::host>
            <For
                each=move || list.get()
                key=|t| t.id
                let:toast
            >
                <div class=move || match toast.kind {
                    ToastKind::Success => format!("{} {}", style::toast, style::success),
                    ToastKind::Error => format!("{} {}", style::toast, style::error),
                    ToastKind::Info => format!("{} {}", style::toast, style::info),
                }>
                    <span>{toast.message.clone()}</span>
                    <button on:click=move |_ev| toasts.dismiss(toast.id)>"×"</button>
                </div>
            </For>
        </div>
    }
}
