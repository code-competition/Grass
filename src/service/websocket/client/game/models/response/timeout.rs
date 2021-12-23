use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::service::websocket::client::models::DefaultModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutResponse {
    pub(crate) d: DefaultModel<Value>,
}

impl TimeoutResponse {
    pub fn new(d: DefaultModel<Value>) -> TimeoutResponse {
        TimeoutResponse { d }
    }
}
