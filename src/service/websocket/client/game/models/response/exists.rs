use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistsResponse {
    pub(crate) exists: bool,
}