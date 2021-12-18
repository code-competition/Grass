use r2d2::Pool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    shards::{self, communication::ShardOpCode},
    websocket::client::models::DefaultModel,
    Sockets,
};

#[derive(Debug, Clone)]
pub struct PartialClient {
    pub(crate) id: Uuid,

    /// true if the player is local to the server\
    /// false if the player is on another shard
    pub(crate) is_local: bool,
}

impl PartialClient {
    pub fn new(id: Uuid, is_local: bool) -> PartialClient {
        PartialClient { id, is_local }
    }

    pub fn send_message<'a, T>(
        &self,
        message: DefaultModel<T>,
        sockets: Option<&Sockets>,
        redis_pool: &Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize + Deserialize<'a>,
    {
        if self.is_local {
            if let Some(sockets) = sockets {
                sockets.get(&self.id).unwrap().send_model(message)?;
            }
        } else {
            shards::send_redis(
                &redis_pool,
                (Some(self.id), None),
                message,
                ShardOpCode::SendAsDefaultModelToClient,
            )?;
        }

        Ok(())
    }
}
