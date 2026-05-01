use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/field_error.module.scss");

#[component]
pub fn FieldError(message: String) -> impl IntoView {
    view! {
        <span class=style::error role="alert">{message}</span>
    }
}
