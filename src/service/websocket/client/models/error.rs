use serde::{Serialize, Deserialize};

use super::OpCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error<'a> {
    pub(crate) err: &'a str,
    pub(crate) code: u32,
}

impl<'a> super::OpCodeFetcher for Error<'a> {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Error
    }
}
