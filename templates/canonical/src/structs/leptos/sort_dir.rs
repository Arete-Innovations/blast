#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    None,
    Asc,
    Desc,
}

impl SortDir {
    pub fn aria_attr(self) -> &'static str {
        match self {
            SortDir::None => "none",
            SortDir::Asc => "ascending",
            SortDir::Desc => "descending",
        }
    }

    pub fn arrow(self) -> &'static str {
        match self {
            SortDir::None => "",
            SortDir::Asc => "\u{25B2}",
            SortDir::Desc => "\u{25BC}",
        }
    }
}
