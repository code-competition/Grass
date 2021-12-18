use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardJoinRequest {
    pub(crate) game_id: String,
    pub(crate) host_id: Uuid,
    pub(crate) client_id: Uuid,
    pub(crate) shard_id: Uuid,
}