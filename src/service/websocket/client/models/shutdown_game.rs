use serde::{Serialize, Deserialize};

use super::OpCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownGame {
    pub(crate) game_id: String,
}

impl super::OpCodeFetcher for ShutdownGame {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::ShutdownGame
    }
}
