use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/form_group.module.scss");

#[component]
pub fn FormGroup(
    label: String,
    #[prop(default = String::new())] for_id: String,
    #[prop(default = None)] error: Option<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let label_stored = StoredValue::new(label);
    let for_id_stored = StoredValue::new(for_id);
    let error_stored = StoredValue::new(error);
    let children_stored = StoredValue::new(children);

    view! {
        <div class=style::group>
            <label class=style::label for=for_id_stored.get_value()>
                {label_stored.get_value()}
            </label>
            <div class=style::control>
                {children_stored.with_value(|c| c())}
            </div>
            {error_stored.with_value(|e| e.clone()).map(|msg| view! {
                <p class=style::error>{msg}</p>
            })}
        </div>
    }
}
