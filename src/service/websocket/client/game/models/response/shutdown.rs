use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownResponse {
    pub(crate) success: bool,
}