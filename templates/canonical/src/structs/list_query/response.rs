use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
    pub total_pages: u64,
}

impl<T> ListResponse<T> {
    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> ListResponse<U> {
        ListResponse {
            items: self.items.into_iter().map(f).collect(),
            page: self.page,
            page_size: self.page_size,
            total: self.total,
            total_pages: self.total_pages,
        }
    }
}
