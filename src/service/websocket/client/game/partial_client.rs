use std::collections::HashMap;

use r2d2::Pool;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::{client::models::DefaultModel, SocketSender},
};

#[derive(Debug, Clone)]
pub struct PartialClient {
    pub(crate) id: Uuid,
    pub(crate) nickname: String,

    /// The shard_id where the client is registered
    pub(crate) shard_id: String,

    /// true if the player is local to the server\
    /// false if the player is on another shard
    pub(crate) is_local: bool,

    /// Only available on local sockets, prevents deadlocking within games
    pub(crate) write_channel: Option<SocketSender>,

    /// Only available if the client is a client in a game
    pub(crate) task_progress: Option<HashMap<usize, bool>>,
}

impl PartialClient {
    pub fn new(
        id: Uuid,
        nickname: String,
        shard_id: String,
        is_local: bool,
        write_channel: Option<SocketSender>,
    ) -> PartialClient {
        PartialClient {
            id,
            nickname,
            shard_id,
            is_local,
            write_channel,
            task_progress: None,
        }
    }

    pub async fn send_message<'a, T>(
        &self,
        message: DefaultModel<T>,
        _redis_pool: &Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize + Deserialize<'a>,
    {
        if self.is_local {
            self.write_channel
                .as_ref()
                .unwrap()
                .send(Message::Text(serde_json::to_string(&message).unwrap()))
                .await?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn shard_id(&self) -> &str {
        self.shard_id.as_ref()
    }
}
