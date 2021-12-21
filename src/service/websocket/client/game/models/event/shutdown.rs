use serde::{Deserialize, Serialize};

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownGameEvent {}

impl GameEventOpCodeFetcher for ShutdownGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::Shutdown
    }
}
