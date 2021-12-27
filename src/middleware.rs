use r2d2::Pool;

use crate::service::{redis_pool::RedisConnectionManager, Sockets};

/// shard_payload_interceptor
///
/// Intercepts messages from other sharding and handles them
pub async fn shard_payload_interceptor(
    _shard_id: String,
    _sockets: Sockets,
    _redis_pool: Pool<RedisConnectionManager>,
    _payload: (),
) {
    // info!(
    //     "Receiving payload from other shard with opcode {:?}",
    //     payload.op
    // );
    // match payload.op {
    //     ShardOpCode::SendAsDefaultModelToClient(client_id) => {
    //         let model = payload.data::<DefaultModelSharding>();
    //         let model: DefaultModel<Value> =
    //             serde_json::from_str(&model).expect("could not parse default model sharding");
    //         if let Some(socket) = sockets.get(&client_id) {
    //             let _ = socket.send_model(model).await;
    //         }
    //     }
    //     ShardOpCode::GameEvent => todo!(),
    //     ShardOpCode::Request => {
    //         let request = payload.data::<ShardRequest>();
    //         match request.handle(shard_id, sockets, redis_pool).await {
    //             Ok(_) => (),
    //             Err(e) => {
    //                 error!("error while handling shard request payload: {}", e);
    //             }
    //         }
    //     }
    //     ShardOpCode::Response => {
    //         let response = payload.data::<ShardResponse>();
    //         match response.handle(shard_id, sockets, redis_pool).await {
    //             Ok(_) => (),
    //             Err(e) => {
    //                 error!("error while handling shard response payload: {}", e);
    //             }
    //         }
    //     }
    // }
}
