#[derive(Debug, Clone)]
pub struct TaskProgress {
    /// If the user's solution works with the first test case
    pub(crate) finished_first: bool,

    // If the user's solution works with all
    pub(crate) finished_all: bool,
}