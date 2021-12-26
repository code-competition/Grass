use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationResponse {
    pub(crate) task_index: usize,
    pub(crate) task_test_progress: Vec<(usize, bool)>,
    pub(crate) is_done: bool,
    pub(crate) is_done_with_public_tests: bool,
    pub(crate) is_done_with_private_tests: bool,
    pub(crate) stderr: String,
}