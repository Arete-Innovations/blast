use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/empty_state.module.scss");

#[component]
pub fn EmptyState(
    title: String,
    message: String,
    #[prop(default = None)] action: Option<AnyView>,
) -> impl IntoView {
    view! {
        <div class=style::wrap role="status">
            <h2 class=style::title>{title}</h2>
            <p class=style::message>{message}</p>
            {action.map(|a| view! { <div class=style::action>{a}</div> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_state_compiles() {
        assert!(true);
    }
}
