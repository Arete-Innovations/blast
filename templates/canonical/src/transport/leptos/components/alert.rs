use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::AlertKind;

import_crate_style!(style, "src/transport/leptos/components/alert.module.scss");

#[component]
pub fn Alert(
    kind: AlertKind,
    #[prop(default = false)] dismissible: bool,
    children: ChildrenFn,
) -> impl IntoView {
    let dismissed = RwSignal::new(false);
    let children_stored = StoredValue::new(children);

    let on_dismiss = move |_ev: leptos::ev::MouseEvent| {
        dismissed.set(true);
    };

    view! {
        <Show when=move || !dismissed.get() fallback=|| ()>
            <div class=style::alert data-kind=kind.as_str() role="alert">
                <div class=style::body>
                    {children_stored.with_value(|c| c())}
                </div>
                <Show when=move || dismissible fallback=|| ()>
                    <button
                        class=style::dismiss
                        on:click=on_dismiss
                        aria-label="Dismiss"
                    >"\u{00d7}"</button>
                </Show>
            </div>
        </Show>
    }
}
