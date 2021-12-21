use std::time::Duration;

use crossbeam::channel::Sender;
use r2d2::Pool;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    sharding::{self, communication::ShardOpCode},
    websocket::client::models::DefaultModel,
};

#[derive(Debug, Clone)]
pub struct PartialClient {
    pub(crate) id: Uuid,

    /// The shard_id where the client is registered
    pub(crate) shard_id: String,

    /// true if the player is local to the server\
    /// false if the player is on another shard
    pub(crate) is_local: bool,

    /// Only available on local sockets, prevents deadlocking within games
    pub(crate) write_channel: Option<Sender<Message>>,
}

impl PartialClient {
    pub fn new(id: Uuid, shard_id: String, is_local: bool, write_channel: Option<Sender<Message>>) -> PartialClient {
        PartialClient {
            id,
            shard_id,
            is_local,
            write_channel,
        }
    }

    pub fn send_message<'a, T>(
        &self,
        message: DefaultModel<T>,
        redis_pool: &Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize + Deserialize<'a>,
    {
        if self.is_local {
            self.write_channel.as_ref().unwrap().send_timeout(
                Message::Text(serde_json::to_string(&message).unwrap()),
                Duration::from_secs(2),
            )?;
        } else {
            sharding::send_redis(
                redis_pool,
                (Some(self.id), None),
                message.to_sharding(),
                ShardOpCode::SendAsDefaultModelToClient(self.id),
            )?;
        }

        Ok(())
    }
}
