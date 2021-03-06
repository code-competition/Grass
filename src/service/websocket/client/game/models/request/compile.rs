use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileRequest {
    pub(crate) code: String,
    pub(crate) task_index: usize,
}
