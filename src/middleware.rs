use r2d2::Pool;
use serde_json::Value;

use crate::service::{
    redis_pool::RedisConnectionManager,
    sharding::communication::{
        request::ShardRequest, response::ShardResponse, ShardDefaultModel, ShardOpCode,
    },
    websocket::client::models::DefaultModel,
    Sockets,
};

/// shard_payload_interceptor
///
/// Intercepts messages from other sharding and handles them
pub fn shard_payload_interceptor(
    sockets: Sockets,
    redis_pool: Pool<RedisConnectionManager>,
    payload: ShardDefaultModel,
) {
    match payload.op {
        ShardOpCode::SendAsDefaultModelToClient(client_id) => {
            let model = payload.data::<DefaultModel<Value>>();
            match sockets.get(&client_id) {
                Some(socket) => {
                    let _ = socket.send_model(model);
                }
                None => (),
            }
        }
        ShardOpCode::GameEvent => todo!(),
        ShardOpCode::Request => {
            let request = payload.data::<ShardRequest>();
            match request.handle(sockets, redis_pool) {
                Ok(_) => (),
                Err(e) => {
                    error!("error while handling shard request payload: {}", e);
                }
            }
        }
        ShardOpCode::Response => {
            let response = payload.data::<ShardResponse>();
            match response.handle(sockets, redis_pool) {
                Ok(_) => (),
                Err(e) => {
                    error!("error while handling shard response payload: {}", e);
                }
            }
        }
    }
}
