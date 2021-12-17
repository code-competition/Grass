use serde::{Serialize, Deserialize};

use crate::service::websocket::client::models::{OpCodeFetcher, OpCode};

pub mod shutdown;

// Models for games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent<T> {
    event: Option<T>,
    op: GameEventOpCode,
}

impl<T> GameEvent<T> {
    pub fn new(event: Option<T>, op: GameEventOpCode) -> Self {
        GameEvent { event, op }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GameEventOpCode {
    /// Event triggered when game ends or host decides to force shutdown it
    Shutdown,
}

impl<T> OpCodeFetcher for GameEvent<T> {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::GameEvent
    }
}
