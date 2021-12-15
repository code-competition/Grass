use serde::{Serialize, Deserialize};

use super::OpCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinGameResponse {
    pub(crate) game_id: String,
    pub(crate) is_host: bool,
    pub(crate) success: bool,
}

impl super::OpCodeFetcher for JoinGameResponse {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::JoinGameResponse
    }
}
