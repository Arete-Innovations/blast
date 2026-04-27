use super::sort::Sort;

#[derive(Default)]
pub struct ListQueryBuilder {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub sort: Vec<Sort>,
    pub filter: Vec<(String, String)>,
}
