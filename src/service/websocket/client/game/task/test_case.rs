use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// stdin that the testcase sends when it runs
    pub(crate) stdin: String,

    /// the output the program is expected to return
    pub(crate) expected: String,

    /// local id
    pub(crate) id: usize,
}