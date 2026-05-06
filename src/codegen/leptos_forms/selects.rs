use crate::state::{FieldState, resource::ValidatorRule};

pub(super) fn find_one_of_options(field: &FieldState) -> Option<&Vec<String>> {
    field.validators.iter().find_map(|v| match v {
        ValidatorRule::OneOf(opts) => Some(opts),
        _other => None,
    })
}

pub(super) fn emit_select<'a>(out: &mut String, name: &str, options: impl Iterator<Item = (&'a str, &'a str)>) {
    out.push_str(&format!("                <select prop:value=move || {name}.get() on:change=move |ev| {name}.set(event_target_value(&ev))>\n"));
    for (value, label) in options {
        let v_esc = value.replace('\\', "\\\\").replace('"', "\\\"");
        let l_esc = label.replace('\\', "\\\\").replace('"', "\\\"");
        out.push_str(&format!("                    <option value=\"{v_esc}\">\"{l_esc}\"</option>\n"));
    }
    out.push_str("                </select>\n");
}
