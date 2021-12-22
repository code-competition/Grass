pub mod test_case;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use self::test_case::TestCase;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTask {
    /// Global id that identifies the task among all existing ones in the database
    task_id: Uuid,

    /// Could be replaced by some formatting thingy
    question: String,

    /// Test cases, are validated with stdout
    test_cases: Vec<TestCase>,
}
