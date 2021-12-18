use r2d2::Pool;
use serde_json::Value;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{error::ClientError, game::models::Request},
    Sockets,
};

use super::{
    models::{DefaultModel, OpCode},
    SocketClient,
};

pub struct ClientMessageHandler {}

impl ClientMessageHandler {
    pub fn handle_message(
        client: &mut SocketClient,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
        model: DefaultModel<Value>,
        shard_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = if let Some(data) = model.d {
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

                return request.handle_message(client, sockets, redis_pool, shard_id);
            }
            _ => {
                return Err(Box::new(ClientError::InvalidOpCode));
            }
        }
    }
}
