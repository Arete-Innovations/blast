use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/percent.module.scss");

#[component]
pub fn PercentCell(value: f64, #[prop(default = 1)] decimals: u8) -> impl IntoView {
    let formatted = format!("{:.prec$}%", value, prec = decimals as usize);
    view! {
        <span class=style::percent>{formatted}</span>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn percent_format() {
        let val = 99.9_f64;
        let s = format!("{:.1}%", val);
        assert_eq!(s, "99.9%");
    }
}
