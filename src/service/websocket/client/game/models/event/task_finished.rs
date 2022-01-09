use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::websocket::client::game::task::GameTask;

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFinishedGameEvent {
    pub(crate) task: GameTask,
    pub(crate) task_index: usize,
    pub(crate) client_id: Uuid,
}

impl GameEventOpCodeFetcher for TaskFinishedGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::TaskFinished
    }
}
