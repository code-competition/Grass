use r2d2::Pool;
use redis::Commands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use self::communication::{ShardDefaultModel, ShardOpCode};

use super::redis_pool::RedisConnectionManager;

pub mod communication;

/// Contains helper functions to send messages across the pub/sub redis channel
pub fn send_redis<'a, T>(
    redis_pool: &Pool<RedisConnectionManager>,
    client_id: Uuid,
    message: T,
    opcode: ShardOpCode,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: Serialize + Deserialize<'a>,
{
    // Serialize message with flexbuffers
    let mut flex_serializer = flexbuffers::FlexbufferSerializer::new();
    message.serialize(&mut flex_serializer).unwrap();

    let message = ShardDefaultModel {
        op: opcode,
        d: Some(flex_serializer.view().to_vec()),
    };

    // fetch redis connection from redis pool
    let mut conn = redis_pool.get()?;

    // Serialize with flexbuffers
    let mut flex_serializer = flexbuffers::FlexbufferSerializer::new();
    message.serialize(&mut flex_serializer).unwrap();

    // Fetch the shard id where the client is registered
    // and send the flexbuffer message to redis channel
    let shard_id: String = conn.get(format!("SOCKET:USER:{}", client_id.to_string()))?;
    let _: () = conn.publish(shard_id, flex_serializer.view())?;

    Ok(())
}