use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardJoinResponse {
    pub(crate) game_id: String,
    pub(crate) host_id: Uuid,
    pub(crate) client_id: Uuid,

    /// Id of shard where the game is hosted
    pub(crate) shard_id: Uuid,
}