use std::fmt::Display;

use serde::{Deserialize, Serialize};

use super::OpCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error<'a> {
    pub(crate) err: &'a str,
}

impl<'a> super::OpCodeFetcher for Error<'a> {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Error
    }
}

impl<'a> Display for Error<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error \"{}\"", self.err)
    }
}

impl<'a> std::error::Error for Error<'a> {}
