use std::sync::Arc;

use r2d2::Pool;
use serde_json::Value;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{error::ClientError, game::models::Request},
    Sockets,
};

use super::{
    game::task::GameTask,
    models::{DefaultModel, OpCode},
};

pub struct ClientMessageHandler {}

impl ClientMessageHandler {
    pub async fn handle_message(
        client_id: Uuid,
        sockets: &Sockets,
        redis_pool: Pool<RedisConnectionManager>,
        available_tasks: Arc<Vec<GameTask>>,
        model: &DefaultModel<Value>,
        shard_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = if let Some(data) = model.d.to_owned() {
            data
        } else {
            return Err(Box::new(ClientError::NoDataWithOpCode(
                "No data was sent with opcode",
            )));
        };

        trace!(
            "Receieved message from client, parsing it OpCode: {:?}",
            &model.op
        );

        match model.op {
            OpCode::Request => {
                let request: Request = serde_json::from_value(data)?;
                request.handle_message(client_id, sockets, redis_pool, available_tasks, shard_id).await
            }
            _ => Err(Box::new(ClientError::InvalidOpCode)),
        }
    }
}
