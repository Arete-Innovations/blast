use leptos::prelude::*;

use crate::transport::leptos::signals::toast::{use_toast, ToastKind};

#[component]
pub fn ToastHost() -> impl IntoView {
    let toasts = use_toast();
    let list = toasts.list();

    view! {
        <div class="toast-host">
            <For
                each=move || list.get()
                key=|t| t.id
                let:toast
            >
                <div class=move || match toast.kind {
                    ToastKind::Success => "toast toast-success",
                    ToastKind::Error => "toast toast-error",
                    ToastKind::Info => "toast toast-info",
                }>
                    <span>{toast.message.clone()}</span>
                    <button on:click=move |_ev| toasts.dismiss(toast.id)>"×"</button>
                </div>
            </For>
        </div>
    }
}
