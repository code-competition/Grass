use serde::{Deserialize, Serialize};

pub use default::{DefaultModel, DefaultModelSharding};

// All models are derived from default
pub mod default;
pub mod hello;
pub mod forced_disconnection;

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
