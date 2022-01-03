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
    pub async fn handle_message<'a>(
        client_id: Uuid,
        sockets: &Sockets,
        redis_pool: Pool<RedisConnectionManager>,
        available_tasks: Arc<Vec<GameTask>>,
        model: &DefaultModel<Value>,
        shard_id: &str,
    ) -> Result<(), ClientError<'a>> {
        superluminal_perf::begin_event("handle message");
        let data = if let Some(data) = model.d.to_owned() {
            data
        } else {
            return Err(ClientError::NoDataWithOpCode(
                "No data was sent with opcode",
            ));
        };

        superluminal_perf::end_event();
        match model.op {
            OpCode::Request => {
                let request: Request =
                    serde_json::from_value(data).map_err(|_| ClientError::ParsingError)?;
                request
                    .handle_message(client_id, sockets, redis_pool, available_tasks, shard_id)
                    .await
            }
            _ => Err(ClientError::InvalidOpCode),
        }
    }
}
