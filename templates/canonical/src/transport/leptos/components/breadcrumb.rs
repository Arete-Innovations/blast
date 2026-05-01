use leptos::prelude::*;
use leptos_router::components::A;
use stylance::import_crate_style;

use crate::structs::leptos::BreadcrumbItem;

import_crate_style!(style, "src/transport/leptos/components/breadcrumb.module.scss");

#[component]
pub fn Breadcrumb(items: Vec<BreadcrumbItem>) -> impl IntoView {
    let last_idx = items.len().saturating_sub(1);
    view! {
        <nav class=style::wrap aria-label="breadcrumb">
            <ol class=style::list>
                {items.into_iter().enumerate().map(|(idx, item)| {
                    let is_last = idx == last_idx;
                    let show_chevron = idx > 0;
                    view! {
                        <li class=style::item>
                            <Show when=move || show_chevron fallback=|| ()>
                                <span class=style::sep aria-hidden="true">"\u{203A}"</span>
                            </Show>
                            {render_crumb(item.clone(), is_last)}
                        </li>
                    }
                }).collect_view()}
            </ol>
        </nav>
    }
}

fn render_crumb(item: BreadcrumbItem, is_last: bool) -> AnyView {
    let label = item.label.clone();
    let route_opt = match is_last {
        true => Option::<crate::structs::leptos::RouteName>::None,
        false => item.to.clone(),
    };
    match route_opt {
        Some(route) => {
            let href = route.path().to_string();
            view! {
                <A href=href>
                    <span class=style::link>{label}</span>
                </A>
            }
            .into_any()
        }
        None => view! {
            <span class=style::current aria-current="page">{label}</span>
        }
        .into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structs::leptos::RouteName;

    #[test]
    fn breadcrumb_linked_item() {
        let item = BreadcrumbItem::linked("Home", RouteName::Dashboard);
        assert_eq!(item.label, "Home");
        assert!(item.to.is_some());
    }

    #[test]
    fn breadcrumb_current_item() {
        let item = BreadcrumbItem::current("Detail");
        assert!(item.to.is_none());
    }
}
