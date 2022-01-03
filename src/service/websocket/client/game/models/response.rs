use serde::{Serialize, Deserialize};

use crate::service::websocket::client::models::{OpCodeFetcher, OpCode};

pub mod join;
pub mod leave;
pub mod shutdown;
pub mod task;
pub mod timeout;
pub mod ping;
pub mod compile;
pub mod identify;
pub mod create;
pub mod exists;

// Models for responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response<T> {
    d: Option<T>,
    op: ResponseOpCode,
}

impl<T> Response<T> {
    pub fn new(d: Option<T>, op: ResponseOpCode) -> Self {
        Response { d, op }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResponseOpCode {
    Join,
    Leave,
    Shutdown,
    Task,
    Timeout,
    Ping,
    Compile,
    Identify,
    Create,
    Exists,
}

impl<T> OpCodeFetcher for Response<T> {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Response
    }
}
