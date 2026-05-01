#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLayout {
    Cards,
    Split,
    Table,
    Bleed,
    Tabbed,
}

impl PageLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            PageLayout::Cards => "cards",
            PageLayout::Split => "split",
            PageLayout::Table => "table",
            PageLayout::Bleed => "bleed",
            PageLayout::Tabbed => "tabbed",
        }
    }
}
