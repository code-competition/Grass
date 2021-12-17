use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownGameEvent {
    pub(crate) game_id: String,
}
