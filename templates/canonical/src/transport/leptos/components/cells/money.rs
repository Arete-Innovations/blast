use leptos::prelude::*;
use stylance::import_crate_style;

use crate::structs::leptos::Currency;

import_crate_style!(style, "src/transport/leptos/components/cells/money.module.scss");

fn format_money(amount: i64, currency: Currency) -> String {
    let decimals = currency.minor_unit_decimals() as i64;
    let divisor = 10_i64.pow(decimals as u32);
    let (sign, abs) = if amount < 0 { ("-", -amount) } else { ("", amount) };
    if decimals == 0 {
        format!("{}{}{}", sign, currency.symbol(), group_thousands(abs))
    } else {
        let major = abs / divisor;
        let minor = abs % divisor;
        format!(
            "{}{}{}.{:0>width$}",
            sign,
            currency.symbol(),
            group_thousands(major),
            minor,
            width = decimals as usize,
        )
    }
}

fn group_thousands(n: i64) -> String {
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    chars
        .iter()
        .enumerate()
        .flat_map(|(i, c)| {
            let remaining = len - i;
            if remaining > 1 && remaining % 3 == 1 {
                Some(',').into_iter().chain(Some(*c))
            } else {
                None.into_iter().chain(Some(*c))
            }
        })
        .collect()
}

#[component]
pub fn MoneyCell(amount: i64, #[prop(default = Currency::Usd)] currency: Currency) -> impl IntoView {
    let display = format_money(amount, currency);
    view! {
        <span class=style::money>{display}</span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usd_format() {
        assert_eq!(format_money(100, Currency::Usd), "$1.00");
        assert_eq!(format_money(1_234_567, Currency::Usd), "$12,345.67");
    }

    #[test]
    fn eur_format() {
        assert_eq!(format_money(5000, Currency::Eur), "\u{20ac}50.00");
    }

    #[test]
    fn jpy_no_decimals() {
        assert_eq!(format_money(1500, Currency::Jpy), "\u{a5}1,500");
    }

    #[test]
    fn negative_amount() {
        assert_eq!(format_money(-999, Currency::Usd), "-$9.99");
    }

    #[test]
    fn zero() {
        assert_eq!(format_money(0, Currency::Usd), "$0.00");
    }
}
