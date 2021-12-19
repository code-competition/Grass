use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveResponse {
    pub(crate) game_id: String,
    pub(crate) success: bool,
}