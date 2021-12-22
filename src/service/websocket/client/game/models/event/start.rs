use serde::{Deserialize, Serialize};

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartGameEvent {
    pub(crate) task_count: usize,
}

impl GameEventOpCodeFetcher for StartGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::Start
    }
}
