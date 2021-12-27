use serde::{Serialize, Deserialize};
use uuid::Uuid;

use super::{GameEventOpCode, GameEventOpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedClientGameEvent {
    pub(crate) game_id: String,
    pub(crate) client_id: Uuid,
    pub(crate) nickname: String,
}

impl GameEventOpCodeFetcher for ConnectedClientGameEvent {
    #[inline]
    fn op_code() -> GameEventOpCode {
        GameEventOpCode::ConnectedClient
    }
}
