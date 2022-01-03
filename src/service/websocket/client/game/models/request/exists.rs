use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistsRequest {
    pub(crate) game_id: String,
}
