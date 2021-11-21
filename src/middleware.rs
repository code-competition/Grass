use std::sync::Arc;

use dashmap::DashMap;
use redis::FromRedisValue;
use uuid::Uuid;

use crate::service::websocket::client::SocketClient;

/// shard_payload_interceptor
/// 
/// Intercepts messages from other shards and handles them
pub fn shard_payload_interceptor<T>(_connections: Arc<DashMap<Uuid, SocketClient>>, payload: T)
where
    T: FromRedisValue + std::fmt::Debug,
{
    info!("Received payload: {:?}", payload);
}