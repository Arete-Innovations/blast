use serde::{Deserialize, Serialize};

use super::direction::SortDirection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    pub column: String,
    pub direction: SortDirection,
}
