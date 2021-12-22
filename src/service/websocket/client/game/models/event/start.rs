use serde::{Deserialize, Serialize};

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartGameEvent {}

impl GameEventOpCodeFetcher for StartGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::Start
    }
}
