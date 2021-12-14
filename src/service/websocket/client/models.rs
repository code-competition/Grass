use num_enum::TryFromPrimitive;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use default::DefaultModel;
pub use hello::Hello;
pub use error::Error;

// All models are derived from default
mod default;
mod hello;
mod error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Hello = 0,
    Error = 1,
}

impl Serialize for OpCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for OpCode {
    fn deserialize<D>(deserializer: D) -> Result<OpCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        OpCode::try_from(value).map_err(serde::de::Error::custom)
    }
}

pub trait OpCodeFetcher {
    fn op_code() -> OpCode;
}
