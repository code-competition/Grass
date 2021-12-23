use r2d2::Pool;
use serde::{Deserialize, Serialize};

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{
        error::ClientError,
        game::{
            models::{
                response::{join::JoinResponse, leave::LeaveResponse},
                Response, ResponseOpCode,
            },
            partial_client::PartialClient,
            Game,
        },
        models::DefaultModel,
    },
    Sockets,
};

use self::{join::ShardJoinResponse, leave::ShardLeaveResponse};

use super::{ShardOpCode, ShardOpCodeFetcher};

pub mod join;
pub mod leave;

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

    pub async fn handle(
        self,
        shard_id: String,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.op {
            ShardResponseOpCode::Join => {
                let response = self.data::<ShardJoinResponse>();

                // Register the game on the client
                let mut s = sockets.write().await;
                let socket = s.get_mut(&response.client_id);
                let mut socket = match socket {
                    Some(socket) => socket,
                    None => {
                        return Err(Box::new(ClientError::ClientDoesNotExist("Client does not exist")));
                    }
                };

                if socket.game.is_some() {
                    socket
                        .send_error(ClientError::AlreadyInGame("Client is already in a game"))
                        .await.map_err(|_| ClientError::SendError)?;
                    return Err(Box::new(ClientError::AlreadyInGame(
                        "Client was for some stupid reason already in a game",
                    )));
                }

                if !response.success {
                    socket
                        .send_model(DefaultModel::new(Response::new(
                            Some(JoinResponse {
                                game_id: response.game_id,
                                is_host: false,
                                success: false,
                            }),
                            ResponseOpCode::Join,
                        )))
                        .await.map_err(|_| ClientError::SendError)?;
                    return Ok(());
                }

                // Client successfully joined the game, give the client its game object
                socket.game = Some(Game::new(
                    false,
                    response.game_id.clone(),
                    PartialClient::new(
                        response.client_id,
                        shard_id,
                        true,
                        Some(socket.send_channel.clone()),
                    ),
                    PartialClient::new(
                        response.host_id,
                        response.shard_id.to_string(),
                        false,
                        None,
                    ),
                    redis_pool,
                    sockets.clone(),
                ));

                socket
                    .send_model(DefaultModel::new(Response::new(
                        Some(JoinResponse {
                            game_id: response.game_id,
                            is_host: false,
                            success: true,
                        }),
                        ResponseOpCode::Join,
                    )))
                    .await.map_err(|_| ClientError::SendError)?;
            }
            ShardResponseOpCode::Leave => {
                let response = self.data::<ShardLeaveResponse>();
                println!("{:?}", (response));

                let s = sockets.read().await;
                let socket = s.get(&response.client_id);
                let socket = match socket {
                    Some(socket) => socket,
                    None => {
                        return Err(Box::new(ClientError::ClientDoesNotExist("Client does not exist")));
                    }
                };

                socket
                    .send_model(DefaultModel::new(Response::new(
                        Some(LeaveResponse { success: true }),
                        ResponseOpCode::Leave,
                    )))
                    .await.map_err(|_| ClientError::SendError)?;
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
    Leave,
}
