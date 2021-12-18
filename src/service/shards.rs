use r2d2::Pool;
use redis::Commands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use self::communication::{ShardDefaultModel, ShardOpCode, ShardOpCodeFetcher};

use super::redis_pool::RedisConnectionManager;

pub mod communication;

/// Contains helper functions to send messages across the pub/sub redis channel
/// Moves the message T into the default ShardDefaultModel and serializes it
pub fn send_redis<'a, T>(
    redis_pool: &Pool<RedisConnectionManager>,
    client_id_or_shard_id: (Option<Uuid>, Option<Uuid>),
    message: T,
    opcode: ShardOpCode,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: ShardOpCodeFetcher + Serialize + Deserialize<'a>,
{
    let message = ShardDefaultModel::new(message, opcode);

    // fetch redis connection from redis pool
    let mut conn = redis_pool.get()?;

    // Serialize with flexbuffers
    let mut flex_serializer = flexbuffers::FlexbufferSerializer::new();
    message.serialize(&mut flex_serializer).unwrap();

    // Fetch the shard id where the client is registered
    // and send the flexbuffer message to redis channel
    let shard_id: String = if let Some(shard_id) = client_id_or_shard_id.1 {
        shard_id.to_string()
    } else {
        conn.get(format!(
            "SOCKET:USER:{}",
            client_id_or_shard_id.0.unwrap().to_string()
        ))?
    };
    let _: () = conn.publish(shard_id, flex_serializer.view())?;

    Ok(())
}
