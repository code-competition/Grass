use r2d2::Pool;

use crate::service::{shards::communication::{ShardDefaultModel, ShardOpCode, request::ShardRequest}, Sockets, redis_pool::RedisConnectionManager};

/// shard_payload_interceptor
///
/// Intercepts messages from other shards and handles them
pub fn shard_payload_interceptor(sockets: Sockets, redis_pool: Pool<RedisConnectionManager>, payload: ShardDefaultModel) {
    match payload.op {
        ShardOpCode::SendAsDefaultModelToClient => {
            
        },
        ShardOpCode::GameEvent => todo!(),
        ShardOpCode::Request => {
            let request = payload.data::<ShardRequest>();
            match request.handle(sockets, redis_pool) {
                Ok(_) => (),
                Err(e) => {
                    error!("error while handling shard payload: {}", e);
                },
            }
        },
        ShardOpCode::Response => {
            
        },
    }
}
