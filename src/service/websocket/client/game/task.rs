pub mod test_case;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod progress;

use self::test_case::TestCase;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTask {
    /// Global id that identifies the task among all existing ones in the database
    pub(crate) task_id: Uuid,

    /// Could be replaced by some formatting thingy
    pub(crate) question: String,

    /// Test cases, are validated with stdout \
    /// First test case is public, rest are run when the client succeeds with the first one
    #[serde(skip_serializing)]
    pub(crate) test_cases: Vec<TestCase>,
}
