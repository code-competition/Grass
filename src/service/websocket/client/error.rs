use std::fmt;

use serde::{Serialize, Deserialize};

use super::models::{OpCodeFetcher, OpCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientError<'a> {
    InvalidMessage(&'a str),
    ClientDoesNotExist(&'a str),
    AlreadyInGame(&'a str),
    NotInGame(&'a str),
    InternalServerError(&'a str),
    NotGameHost(&'a str),
    NoDataWithOpCode(&'a str),
    OutOfRangeTask,
    NoGameWasFound,
    GameNotStarted,
    InvalidGameID,
    InvalidOpCode,
    ParsingError,
    SendError,
}

impl<'a> OpCodeFetcher for ClientError<'a> {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Error
    }
}

impl<'a> fmt::Display for ClientError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error \"{:?}\"", self)
    }
}

impl<'a> std::error::Error for ClientError<'a> {}