#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLayout {
    Cards,
    Split,
    Table,
    Bleed,
    Tabbed,
}

impl PageLayout {
    pub fn class(self) -> &'static str {
        match self {
            PageLayout::Cards => "page-shell layout-cards",
            PageLayout::Split => "page-shell layout-split",
            PageLayout::Table => "page-shell layout-table",
            PageLayout::Bleed => "page-shell layout-bleed",
            PageLayout::Tabbed => "page-shell layout-tabbed",
        }
    }
}
