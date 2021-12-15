use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisGame {
    pub(crate) shard_id: String,
    pub(crate) host_id: Uuid,
}