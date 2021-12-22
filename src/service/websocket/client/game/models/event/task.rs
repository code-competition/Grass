use serde::{Deserialize, Serialize};

use crate::service::websocket::client::game::task::GameTask;

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGameEvent {
    pub(crate) task: GameTask,
}

impl GameEventOpCodeFetcher for TaskGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::Task
    }
}
