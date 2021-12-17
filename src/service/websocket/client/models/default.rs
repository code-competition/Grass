use serde_derive::{Deserialize, Serialize};

use super::{OpCode, OpCodeFetcher};

// Model is to be converted into JSON when serialized before sending to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultModel<T> {
    pub(crate) op: OpCode,
    pub(crate) d: Option<T>,
}

impl<T> DefaultModel<T> {
    pub fn new(d: T) -> Self
    where
        T: OpCodeFetcher,
    {
        let op = T::op_code();

        DefaultModel { op, d: Some(d) }
    }
}
