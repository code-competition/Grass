use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicTestProgress {
    pub(crate) test_index: usize,
    pub(crate) succeeded: bool,
    pub(crate) stdout: String,
    pub(crate) expected: String,
}

impl PublicTestProgress {
    pub fn new(test_index: usize, stdout: String, expected: String) -> Self {
        PublicTestProgress {
            test_index,
            succeeded: true,
            stdout,
            expected
        }
    }

    pub fn new_failed(test_index: usize, stdout: String, expected: String) -> Self {
        PublicTestProgress {
            test_index,
            succeeded: false,
            stdout,
            expected
        }
    }

    pub fn make_not_failed(&mut self) {
        self.succeeded = true;
    }
}
