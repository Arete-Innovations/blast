use crate::structs::list_query::sort::Sort;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub page: u32,
    pub page_size: u32,
    pub sort: Vec<Sort>,
    pub filter: Vec<(String, String)>,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 25,
            sort: Vec::new(),
            filter: Vec::new(),
        }
    }
}
