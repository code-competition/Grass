use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownGame {
    pub(crate) game_id: String,
}
