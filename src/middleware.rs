use std::sync::Arc;

use dashmap::DashMap;
use redis::FromRedisValue;
use uuid::Uuid;

use crate::service::websocket::client::SocketClient;

pub fn presence_package_interceptor<T>(_connections: Arc<DashMap<Uuid, SocketClient>>, _payload: T)
where
    T: FromRedisValue,
{

}
