use leptos::prelude::*;
use leptos_router::components::A;
use stylance::import_crate_style;

use crate::structs::leptos::RouteName;

import_crate_style!(style, "src/transport/leptos/components/link_cell.module.scss");

#[component]
pub fn LinkCell(to: RouteName, text: String) -> impl IntoView {
    let href = to.path().to_string();
    view! {
        <A href=href>
            <span class=style::link>{text}</span>
        </A>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_cell_compiles() {
        let _ = RouteName::Dashboard;
    }
}
