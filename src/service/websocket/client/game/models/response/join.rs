use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinResponse {
    pub(crate) game_id: String,
    pub(crate) is_host: bool,
    pub(crate) success: bool,
}