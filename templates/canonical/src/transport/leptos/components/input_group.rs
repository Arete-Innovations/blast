use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/input_group.module.scss");

#[component]
pub fn InputGroup(
    #[prop(default = None)] prefix: Option<String>,
    #[prop(default = None)] suffix: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let prefix_stored = StoredValue::new(prefix);
    let suffix_stored = StoredValue::new(suffix);
    let children_stored = StoredValue::new(children);

    view! {
        <div class=style::group>
            {prefix_stored.with_value(|p| p.clone()).map(|text| view! {
                <span class=style::affix data-position="prefix">{text}</span>
            })}
            <div class=style::control>
                {children_stored.with_value(|c| c())}
            </div>
            {suffix_stored.with_value(|s| s.clone()).map(|text| view! {
                <span class=style::affix data-position="suffix">{text}</span>
            })}
        </div>
    }
}
