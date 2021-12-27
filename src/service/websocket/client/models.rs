use serde::{Deserialize, Serialize};

pub use default::DefaultModel;

// All models are derived from default
pub mod default;
pub mod forced_disconnection;
pub mod hello;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OpCode {
    Hello,
    Error,
    ForcedDisconnection,
    GameEvent,
    Request,
    Response,
}

pub trait OpCodeFetcher {
    fn op_code() -> OpCode;
}
