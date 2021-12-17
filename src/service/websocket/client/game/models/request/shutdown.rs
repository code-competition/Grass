use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownRequest {
    pub(crate) game_id: String,
}