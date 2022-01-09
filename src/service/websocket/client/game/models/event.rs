use serde::{Deserialize, Serialize};

use crate::service::websocket::client::models::{OpCode, OpCodeFetcher};

pub mod shutdown;
pub mod start;
pub mod task;
pub mod task_finished;

/// Event sent to everyone in a game when a new client is connected (not sent to the client itself)
pub mod connected_client;
/// Event sent to everyone in a game when an existing client is disconnected (not sent to the client itself)
pub mod disconnected_client;

// Models for games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEvent<T> {
    event: T,
    op: GameEventOpCode,
}

impl<T> GameEvent<T> {
    pub fn new(event: T) -> Self
    where
        T: GameEventOpCodeFetcher,
    {
        GameEvent {
            op: T::op_code(),
            event,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GameEventOpCode {
    /// Event triggered when game ends or host decides to force shutdown it
    Shutdown,

    Start,
    Task,

    TaskFinished,

    /// Event sent to everyone in a game when a new client is connected (not sent to the client itself)
    ConnectedClient,
    /// Event sent to everyone in a game when an existing client is disconnected (not sent to the client itself)
    DisconnectedClient,
}

pub trait GameEventOpCodeFetcher {
    fn op_code() -> GameEventOpCode;
}

impl<T> OpCodeFetcher for GameEvent<T> {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::GameEvent
    }
}
