use serde::{Serialize, Deserialize};

// Used for shard to shard communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardDefaultModel {
    pub(crate) op: ShardOpCode,
    pub(crate) d: Option<Vec<u8>>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ShardOpCode {
    SendAsDefaultModelToClient = 0,
    GameEvent = 1,
}