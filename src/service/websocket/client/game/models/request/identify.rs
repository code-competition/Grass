use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifyRequest {
    pub(crate) nickname: String,
}