use serde::{Serialize, Deserialize};

use crate::service::websocket::client::game::task::GameTask;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub(crate) task: GameTask
}