use r2d2::Pool;
use serde::{Deserialize, Serialize};

use crate::service::{
    error::ServiceError, redis_pool::RedisConnectionManager,
    websocket::client::game::partial_client::PartialClient, Sockets, shards,
};

use self::join::ShardJoinRequest;

use super::{
    response::{join::ShardJoinResponse, ShardResponse, ShardResponseOpCode},
    ShardOpCode, ShardOpCodeFetcher,
};

pub mod join;

// Models for requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardRequest {
    pub(crate) d: Option<Vec<u8>>,
    pub(crate) op: ShardRequestOpCode,
}

impl ShardRequest {
    pub fn new<'a, T>(d: T, op: ShardRequestOpCode) -> Self
    where
        T: Serialize + Deserialize<'a>,
    {
        // Serialize message with flexbuffers
        let mut flex_serializer = flexbuffers::FlexbufferSerializer::new();
        d.serialize(&mut flex_serializer).unwrap();

        Self {
            d: Some(flex_serializer.view().to_vec()),
            op,
        }
    }

    pub fn data<'a, T>(&'a self) -> T
    where
        T: Serialize + Deserialize<'a>,
    {
        let d = self.d.as_ref().unwrap();
        let r = flexbuffers::Reader::get_root(d.as_slice()).unwrap();
        T::deserialize(r).unwrap()
    }

    pub fn handle(
        self,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.op {
            ShardRequestOpCode::Join => {
                let shard_join_game = self.data::<ShardJoinRequest>();

                // Register the client on the host
                let mut game_host = sockets
                    .get_mut(&shard_join_game.host_id)
                    .ok_or(ServiceError::CouldNotGetSocket)?;
                let game = game_host
                    .game
                    .as_mut()
                    .ok_or(ServiceError::GameDoesNotExist)?;
                game.register(PartialClient::new(shard_join_game.client_id, false));
                drop(game_host);

                // Serialize response
                let response = ShardResponse::new(
                    ShardJoinResponse {
                        game_id: shard_join_game.game_id,
                        host_id: shard_join_game.host_id,
                        client_id: shard_join_game.client_id,
                        shard_id: shard_join_game.shard_id,
                    },
                    ShardResponseOpCode::Join,
                );

                // Send response to shard
                shards::send_redis(
                    &redis_pool,
                    (Some(shard_join_game.client_id), None),
                    response,
                    ShardOpCode::Response,
                )?;
            }
        }

        Ok(())
    }
}

impl ShardOpCodeFetcher for ShardRequest {
    fn op_code() -> ShardOpCode {
        ShardOpCode::Request
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShardRequestOpCode {
    Join,
}
