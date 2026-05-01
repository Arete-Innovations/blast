use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/card.module.scss");

#[component]
pub fn Card(
    #[prop(default = None)] title: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <section class=style::card>
            {title.map(|t| view! {
                <header class=style::header>
                    <h3 class=style::title>{t}</h3>
                </header>
            })}
            <div class=style::body>{children()}</div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn card_compiles() {
        assert!(true);
    }
}
