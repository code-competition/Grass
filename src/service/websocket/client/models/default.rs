use serde_derive::{Deserialize, Serialize};

use super::{OpCode, OpCodeFetcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultModel<T> {
    op: OpCode,
    d: T
}

impl<T> DefaultModel<T> where T: OpCodeFetcher {
    pub fn new(d: T) -> Self {
        let op = T::op_code();

        DefaultModel {
            op,
            d
        }
    }
} 