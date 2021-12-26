pub mod test_case;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use self::test_case::TestCase;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTask {
    /// Global id that identifies the task among all existing ones in the database
    pub(crate) task_id: Uuid,

    /// Could be replaced by some formatting thingy
    pub(crate) question: String,

    /// Public test cases
    pub(crate) public_test_cases: Vec<TestCase>,

    /// Test cases, are validated with stdout
    #[serde(skip_serializing)]
    pub(crate) private_test_cases: Vec<TestCase>,
}
