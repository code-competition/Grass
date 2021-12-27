use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifyResponse {
    pub(crate) success: bool,
}