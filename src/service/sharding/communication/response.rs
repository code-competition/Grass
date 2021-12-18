use r2d2::Pool;
use serde::{Deserialize, Serialize};

use crate::service::{
    error::ServiceError, redis_pool::RedisConnectionManager, websocket::client::{game::{Game, models::{Response, response::join::JoinResponse, ResponseOpCode}}, models::DefaultModel, error::ClientError}, Sockets,
};

use self::join::ShardJoinResponse;

use super::{ShardOpCode, ShardOpCodeFetcher};

pub mod join;

// Models for response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardResponse {
    pub(crate) d: Option<Vec<u8>>,
    pub(crate) op: ShardResponseOpCode,
}

impl ShardResponse {
    pub fn new<'a, T>(d: T, op: ShardResponseOpCode) -> Self
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
            ShardResponseOpCode::Join => {
                let shard_join_game = self.data::<ShardJoinResponse>();

                // Register the game on the client
                let socket = sockets
                    .get_mut(&shard_join_game.client_id);
                let mut socket = match socket {
                    Some(socket) => socket,
                    None => {
                        return Err(Box::new(ServiceError::CouldNotGetSocket));
                    },
                };
                if socket.game.is_some() {
                    socket.send_error(ClientError::AlreadyInGame("Client is already in a game"))?;
                }

                socket.game = Some(Game::new(
                    false,
                    shard_join_game.game_id.clone(),
                    shard_join_game.client_id,
                    shard_join_game.host_id,
                    sockets.clone(),
                    redis_pool.clone(),
                ));

                socket.send_model(DefaultModel::new(Response::new(
                    Some(JoinResponse {
                        game_id: shard_join_game.game_id,
                        is_host: false,
                        success: true,
                    }),
                    ResponseOpCode::Join,
                )))?;
            }
        };

        Ok(())
    }
}

impl ShardOpCodeFetcher for ShardResponse {
    fn op_code() -> ShardOpCode {
        ShardOpCode::Request
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShardResponseOpCode {
    Join,
}
