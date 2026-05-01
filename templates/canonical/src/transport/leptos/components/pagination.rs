use leptos::prelude::*;
use stylance::import_crate_style;

use crate::transport::leptos::signals::use_url_list_state;

import_crate_style!(style, "src/transport/leptos/components/pagination.module.scss");

#[component]
pub fn Pagination(total_pages: u64, current_page: u64) -> impl IntoView {
    let url_state = use_url_list_state();

    let go_first = move |_| url_state.page.set(1);
    let go_prev = {
        let p = current_page;
        move |_| {
            let target = match p > 1 {
                true => p - 1,
                false => 1,
            };
            url_state.page.set(target);
        }
    };
    let go_next = {
        let p = current_page;
        let total = total_pages;
        move |_| {
            let target = match p < total {
                true => p + 1,
                false => total,
            };
            url_state.page.set(target);
        }
    };
    let go_last = {
        let total = total_pages;
        move |_| url_state.page.set(total)
    };

    let prev_disabled = current_page <= 1;
    let next_disabled = current_page >= total_pages;
    let visible = total_pages > 1;
    let label = format!("Page {} of {}", current_page, total_pages);

    view! {
        <Show when=move || visible fallback=|| view! { <span class=style::hidden></span> }>
            <nav class=style::wrap aria-label="pagination">
                <button
                    class=style::btn
                    type="button"
                    on:click=go_first.clone()
                    disabled=prev_disabled
                    aria-label="first page"
                >
                    "<<"
                </button>
                <button
                    class=style::btn
                    type="button"
                    on:click=go_prev.clone()
                    disabled=prev_disabled
                    aria-label="previous page"
                >
                    "<"
                </button>
                <span class=style::label>{label.clone()}</span>
                <button
                    class=style::btn
                    type="button"
                    on:click=go_next.clone()
                    disabled=next_disabled
                    aria-label="next page"
                >
                    ">"
                </button>
                <button
                    class=style::btn
                    type="button"
                    on:click=go_last.clone()
                    disabled=next_disabled
                    aria-label="last page"
                >
                    ">>"
                </button>
            </nav>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pagination_compiles() {
        assert!(true);
    }
}
