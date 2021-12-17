use num_enum::TryFromPrimitive;
use serde::{Serialize, Deserialize, Serializer, Deserializer};

// Used for shard to shard communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardDefaultModel<T> {
    pub(crate) op: ShardOpCode,
    pub(crate) d: Option<T>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(u8)]
pub enum ShardOpCode {
    SendAsDefaultModelToClient = 0,
    GameEvent = 1,
}

impl Serialize for ShardOpCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for ShardOpCode {
    fn deserialize<D>(deserializer: D) -> Result<ShardOpCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        ShardOpCode::try_from(value).map_err(serde::de::Error::custom)
    }
}