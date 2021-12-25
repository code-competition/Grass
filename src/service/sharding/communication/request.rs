use std::str::FromStr;

use r2d2::Pool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager, sharding,
    websocket::client::{game::partial_client::PartialClient, error::ClientError}, Sockets,
};

use self::{join::ShardJoinRequest, leave::ShardLeaveRequest};

use super::{
    response::{
        join::ShardJoinResponse, leave::ShardLeaveResponse, ShardResponse, ShardResponseOpCode,
    },
    ShardOpCode, ShardOpCodeFetcher,
};

pub mod join;
pub mod leave;

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

    pub async fn handle(
        self,
        shard_id: String,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.op {
            ShardRequestOpCode::Join => {
                let request = self.data::<ShardJoinRequest>();

                // Register the client on the host
                if sockets.get_mut(&request.host_id)
                    .ok_or(ClientError::ClientDoesNotExist("Client does not exist"))?
                    .game
                    .as_mut()
                    .ok_or(ClientError::NoGameWasFound)?
                    .register(PartialClient::new(
                        request.client_id,
                        request.shard_id.to_string(),
                        false,
                        None,
                    ))
                    .await
                    .is_err()
                {
                    // Serialize response
                    let response = ShardResponse::new(
                        ShardJoinResponse {
                            game_id: request.game_id,
                            host_id: request.host_id,
                            client_id: request.client_id,
                            shard_id: Uuid::from_str(&shard_id).unwrap(),
                            success: false,
                        },
                        ShardResponseOpCode::Join,
                    );

                    // Send response to shard
                    sharding::send_redis(
                        &redis_pool,
                        (Some(request.client_id), None),
                        response,
                        ShardOpCode::Response,
                    )?;
                    return Ok(());
                }

                // Serialize response
                let response = ShardResponse::new(
                    ShardJoinResponse {
                        game_id: request.game_id,
                        host_id: request.host_id,
                        client_id: request.client_id,
                        shard_id: Uuid::from_str(&shard_id).unwrap(),
                        success: true,
                    },
                    ShardResponseOpCode::Join,
                );

                // Send response to shard
                sharding::send_redis(
                    &redis_pool,
                    (Some(request.client_id), None),
                    response,
                    ShardOpCode::Response,
                )?;
            }
            ShardRequestOpCode::Leave => {
                let request = self.data::<ShardLeaveRequest>();

                // Register the client on the host
                let mut game_host = sockets
                    .get_mut(&request.host_id)
                    .ok_or(ClientError::ClientDoesNotExist("Client does not exist"))?;
                let game = game_host
                    .game
                    .as_mut()
                    .ok_or(ClientError::ClientDoesNotExist("Client does not exist"))?;
                game.unregister(&request.client_id).await;

                // Serialize response
                let response = ShardResponse::new(
                    ShardLeaveResponse {
                        game_id: request.game_id,
                        host_id: request.host_id,
                        client_id: request.client_id,
                        shard_id: Uuid::from_str(&shard_id).unwrap(),
                    },
                    ShardResponseOpCode::Leave,
                );

                // Send response to shard
                sharding::send_redis(
                    &redis_pool,
                    (Some(request.client_id), None),
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
    Leave,
}
