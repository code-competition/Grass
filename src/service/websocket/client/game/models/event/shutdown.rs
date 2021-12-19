use serde::{Serialize, Deserialize};

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownGameEvent {
    pub(crate) game_id: String,
}

impl GameEventOpCodeFetcher for ShutdownGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::Shutdown
    }
}