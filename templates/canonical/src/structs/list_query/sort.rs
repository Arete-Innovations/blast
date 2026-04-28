use serde::{Deserialize, Serialize};

use crate::structs::list_query::direction::SortDirection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    pub column: String,
    pub direction: SortDirection,
}
