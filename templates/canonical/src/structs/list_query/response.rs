use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub total_pages: u64,
}
