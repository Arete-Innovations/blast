#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    Iso,
    Short,
    Long,
    Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoolVariant {
    #[default]
    Check,
    YesNo,
    Badge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeColor {
    #[default]
    Default,
    Success,
    Warning,
    Danger,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Usd,
    Eur,
    Gbp,
    Jpy,
    Cad,
    Aud,
    Chf,
    Cny,
    Inr,
    Brl,
}

impl Currency {
    pub fn symbol(self) -> &'static str {
        match self {
            Currency::Usd => "$",
            Currency::Eur => "\u{20ac}",
            Currency::Gbp => "\u{a3}",
            Currency::Jpy => "\u{a5}",
            Currency::Cad => "CA$",
            Currency::Aud => "A$",
            Currency::Chf => "Fr",
            Currency::Cny => "\u{a5}",
            Currency::Inr => "\u{20b9}",
            Currency::Brl => "R$",
        }
    }

    pub fn minor_unit_decimals(self) -> u8 {
        match self {
            Currency::Jpy => 0,
            Currency::Usd
            | Currency::Eur
            | Currency::Gbp
            | Currency::Cad
            | Currency::Aud
            | Currency::Chf
            | Currency::Cny
            | Currency::Inr
            | Currency::Brl => 2,
        }
    }
}
