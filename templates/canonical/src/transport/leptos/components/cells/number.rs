use leptos::prelude::*;
use stylance::import_crate_style;

import_crate_style!(style, "src/transport/leptos/components/cells/number.module.scss");

fn split_sign(s: &str) -> (&str, &str) {
    match s.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", s),
    }
}

fn format_number(value: f64, decimals: u8, thousands: bool) -> String {
    let prec = decimals as usize;
    let raw = format!("{:.prec$}", value, prec = prec);
    if !thousands {
        return raw;
    }
    match raw.split_once('.') {
        None => {
            let (sign, digits) = split_sign(&raw);
            format!("{}{}", sign, group_int_str(digits))
        }
        Some((int_part, dec_part)) => {
            let (sign, digits) = split_sign(int_part);
            format!("{}{}.{}", sign, group_int_str(digits), dec_part)
        }
    }
}

fn group_int_str(digits: &str) -> String {
    let chars: Vec<char> = digits.chars().collect();
    let len = chars.len();
    chars
        .iter()
        .enumerate()
        .flat_map(|(i, c)| {
            let remaining = len - i;
            if i > 0 && remaining % 3 == 0 {
                Some(',').into_iter().chain(Some(*c))
            } else {
                None.into_iter().chain(Some(*c))
            }
        })
        .collect()
}

#[component]
pub fn NumberCell(
    value: f64,
    #[prop(default = 2)] decimals: u8,
    #[prop(default = true)] thousands: bool,
) -> impl IntoView {
    let formatted = format_number(value, decimals, thousands);
    view! {
        <span class=style::number>{formatted}</span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separator() {
        assert_eq!(format_number(1234567.89, 2, true), "1,234,567.89");
    }

    #[test]
    fn no_thousands_separator() {
        assert_eq!(format_number(1234.5, 1, false), "1234.5");
    }

    #[test]
    fn negative_value() {
        assert_eq!(format_number(-9876.0, 0, true), "-9,876");
    }
}
