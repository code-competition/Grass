use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

use super::OpCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    pub id: Uuid,
}

impl super::OpCodeFetcher for Hello {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Hello
    }
}
