use crate::service::Sockets;
use redis::FromRedisValue;

/// shard_payload_interceptor
///
/// Intercepts messages from other shards and handles them
pub fn shard_payload_interceptor<T>(_connections: Sockets, payload: T)
where
    T: FromRedisValue + std::fmt::Debug,
{
    info!("Received payload: {:?}", payload);
}
