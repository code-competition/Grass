use serde_derive::{Deserialize, Serialize};

use super::OpCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForcedDisconnection {}

impl super::OpCodeFetcher for ForcedDisconnection {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::ForcedDisconnection
    }
}
