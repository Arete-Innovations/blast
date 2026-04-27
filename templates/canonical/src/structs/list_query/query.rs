use super::sort::Sort;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub page: u32,
    pub page_size: u32,
    pub sort: Vec<Sort>,
    pub filter: Vec<(String, String)>,
}
