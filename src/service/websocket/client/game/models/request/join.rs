use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    pub(crate) game_id: String,
}