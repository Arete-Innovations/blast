use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::SortDir;
use crate::transport::leptos::signals::use_url_list_state;

import_crate_style!(style, "src/transport/leptos/components/sort_header.module.scss");

fn parse_first_sort(raw: Option<&str>) -> (Option<String>, SortDir) {
    let s = match raw {
        Some(v) => v,
        None => return (Option::<String>::None, SortDir::None),
    };
    let first = match s.split(',').next() {
        Some(seg) => seg.trim(),
        None => return (Option::<String>::None, SortDir::None),
    };
    if first.is_empty() {
        return (Option::<String>::None, SortDir::None);
    }
    match first.strip_prefix('-') {
        Some(col) => match col.is_empty() {
            true => (Option::<String>::None, SortDir::None),
            false => (Some(col.to_string()), SortDir::Desc),
        },
        None => (Some(first.to_string()), SortDir::Asc),
    }
}

fn next_sort_string(col: &str, current: SortDir) -> Option<String> {
    match current {
        SortDir::None => Some(col.to_string()),
        SortDir::Asc => Some(format!("-{}", col)),
        SortDir::Desc => Option::<String>::None,
    }
}

fn dir_for_column(active: Option<String>, dir: SortDir, col: &str) -> SortDir {
    let active_col = match active {
        Some(v) => v,
        None => return SortDir::None,
    };
    match active_col == col {
        true => dir,
        false => SortDir::None,
    }
}

#[component]
pub fn SortHeader(col: &'static str, label: &'static str) -> impl IntoView {
    let url_state = use_url_list_state();

    let dir_signal = Memo::new(move |_| {
        let raw = url_state.sort.get();
        let (active_col, dir) = parse_first_sort(raw.as_deref());
        dir_for_column(active_col, dir, col)
    });

    let on_click = move |_| {
        let raw = url_state.sort.get_untracked();
        let (active_col, dir) = parse_first_sort(raw.as_deref());
        let current = dir_for_column(active_col, dir, col);
        url_state.sort.set(next_sort_string(col, current));
        url_state.page.set(1);
    };

    let arrow = move || dir_signal.get().arrow();
    let aria_sort = move || dir_signal.get().aria_attr();

    view! {
        <th class=style::th aria-sort=aria_sort>
            <button class=style::btn type="button" on:click=on_click>
                <span class=style::label>{label}</span>
                <span class=style::arrow aria-hidden="true">{arrow}</span>
            </button>
        </th>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_asc() {
        let (c, d) = parse_first_sort(Some("name"));
        assert_eq!(c.as_deref(), Some("name"));
        assert_eq!(d, SortDir::Asc);
    }

    #[test]
    fn parse_desc() {
        let (c, d) = parse_first_sort(Some("-name"));
        assert_eq!(c.as_deref(), Some("name"));
        assert_eq!(d, SortDir::Desc);
    }

    #[test]
    fn parse_empty() {
        let (c, d) = parse_first_sort(None);
        assert!(c.is_none());
        assert_eq!(d, SortDir::None);
    }

    #[test]
    fn cycle_none_to_asc() {
        let next = next_sort_string("col", SortDir::None);
        assert_eq!(next.as_deref(), Some("col"));
    }

    #[test]
    fn cycle_asc_to_desc() {
        let next = next_sort_string("col", SortDir::Asc);
        assert_eq!(next.as_deref(), Some("-col"));
    }

    #[test]
    fn cycle_desc_to_none() {
        let next = next_sort_string("col", SortDir::Desc);
        assert!(next.is_none());
    }
}
