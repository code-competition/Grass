use serde::{Serialize, Deserialize};

use super::OpCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinGame {
    pub(crate) game_id: String,
    pub(crate) acquire_host: bool,
}

impl super::OpCodeFetcher for JoinGame {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::JoinGame
    }
}
