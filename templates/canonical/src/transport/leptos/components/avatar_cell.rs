use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::AvatarSize;

import_crate_style!(style, "src/transport/leptos/components/avatar_cell.module.scss");

fn initials_for(name: &str) -> String {
    let mut out = String::with_capacity(2);
    for word in name.split_whitespace().take(2) {
        match word.chars().next() {
            Some(c) => {
                for u in c.to_uppercase() {
                    out.push(u);
                }
            }
            None => {}
        }
    }
    if out.is_empty() {
        out.push('?');
    }
    out
}

fn non_empty_url(url: Option<String>) -> Option<String> {
    let raw = match url {
        Some(s) => s,
        None => return Option::<String>::None,
    };
    match raw.is_empty() {
        true => Option::<String>::None,
        false => Some(raw),
    }
}

#[component]
pub fn AvatarCell(
    name: String,
    #[prop(default = None)] url: Option<String>,
    #[prop(default = AvatarSize::Md)] size: AvatarSize,
) -> impl IntoView {
    let initials = initials_for(&name);
    let alt = name.clone();
    let size_attr = size.as_str();
    let url_clean = non_empty_url(url);
    view! {
        <span class=style::avatar data-size=size_attr>
            {match url_clean {
                Some(src) => view! {
                    <img class=style::img src=src alt=alt/>
                }
                .into_any(),
                None => view! {
                    <span class=style::initials>{initials}</span>
                }
                .into_any(),
            }}
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_two_words() {
        assert_eq!(initials_for("Jane Doe"), "JD");
    }

    #[test]
    fn initials_single_word() {
        assert_eq!(initials_for("alice"), "A");
    }

    #[test]
    fn initials_empty() {
        assert_eq!(initials_for(""), "?");
    }

    #[test]
    fn sizes_are_distinct() {
        assert_ne!(AvatarSize::Sm.as_str(), AvatarSize::Lg.as_str());
    }
}
