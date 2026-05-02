use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::ButtonKind;

import_crate_style!(style, "src/transport/leptos/components/button.module.scss");

fn classes_for(kind: ButtonKind, full: bool, compact: bool) -> String {
    let variant = match kind {
        ButtonKind::Primary => style::primary,
        ButtonKind::Danger => style::danger,
        ButtonKind::Ghost => style::ghost,
        ButtonKind::Secondary => "",
    };
    let mut out = String::from(style::btn);
    if !variant.is_empty() {
        out.push(' ');
        out.push_str(variant);
    }
    if full {
        out.push(' ');
        out.push_str(style::full);
    }
    if compact {
        out.push(' ');
        out.push_str(style::compact);
    }
    out
}

#[component]
pub fn Button(
    #[prop(default = ButtonKind::Secondary)] kind: ButtonKind,
    #[prop(default = "button".to_string())] kind_attr: String,
    #[prop(default = false)] full: bool,
    #[prop(default = false)] compact: bool,
    #[prop(default = false)] disabled: bool,
    children: Children,
) -> impl IntoView {
    let class = classes_for(kind, full, compact);
    view! {
        <button class=class type=kind_attr disabled=disabled>
            {children()}
        </button>
    }
}

#[component]
pub fn LinkButton(
    href: String,
    #[prop(default = ButtonKind::Secondary)] kind: ButtonKind,
    #[prop(default = false)] full: bool,
    #[prop(default = false)] compact: bool,
    children: Children,
) -> impl IntoView {
    let class = classes_for(kind, full, compact);
    view! {
        <a class=class href=href>{children()}</a>
    }
}
