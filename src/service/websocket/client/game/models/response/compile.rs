use serde::{Serialize, Deserialize};

use self::progress::PublicTestProgress;

pub mod progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationResponse {
    pub(crate) task_index: usize,
    pub(crate) public_test_progress: Vec<PublicTestProgress>,
    pub(crate) is_done: bool,
    pub(crate) is_done_public_tests: bool,
    pub(crate) is_done_private_tests: bool,
    pub(crate) stderr: String,
}