#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    NotAnInt { key: &'static str, value: String },
    PageMustBePositive,
    PageSizeMustBePositive,
    EmptySortSegment,
    InvalidColumnIdent(String),
    MalformedFilterKey(String),
    EmptyFilterColumn,
    UnknownKey(String),
}
