use serde::{Serialize, Deserialize};
use uuid::Uuid;

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectedClientGameEvent {
    pub(crate) game_id: String,

    // ! Todo: Replace client id with a name, we don't want to share client ids to other connected clients
    pub(crate) client_id: Uuid,
}

impl GameEventOpCodeFetcher for DisconnectedClientGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::DisconnectedClient
    }
}