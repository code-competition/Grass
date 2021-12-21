use std::str::FromStr;

use r2d2::Pool;
use redis::Commands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    sharding::{
        self,
        communication::{
            request::{join::ShardJoinRequest, ShardRequest, ShardRequestOpCode},
            ShardOpCode,
        },
    },
    websocket::client::{
        error::ClientError,
        game::{partial_client::PartialClient, redis_game::RedisGame, Game},
        models::{DefaultModel, OpCode, OpCodeFetcher},
    },
    Sockets,
};

use self::join::JoinRequest;

use super::{response::join::JoinResponse, Response, ResponseOpCode};

pub mod join;
pub mod leave;

// Models for requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    d: Option<Value>,
    op: RequestOpCode,
}

impl Request {
    pub fn handle_message(
        self,
        client_id: Uuid,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
        shard_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.op {
            RequestOpCode::Join => {
                if self.d.is_none() {
                    return Err(Box::new(ClientError::InternalServerError(
                        "Internal Server Error",
                    )));
                }

                // Fetch client
                // ! Important: This client is cloned to guarantee that no deadlocks occur
                let client = sockets.get(&client_id).unwrap().clone();

                // Check if user is already in a game
                if client.game.is_some() {
                    let _ = client
                        .send_error(ClientError::AlreadyInGame("Client is already in a game"));
                    return Err(Box::new(ClientError::AlreadyInGame(
                        "Client is already in a game",
                    )));
                }

                // Parse the join request
                let join_game: JoinRequest = serde_json::from_value(self.d.unwrap())?;

                // get a redis connection from the pool
                let conn = redis_pool.get();
                let mut conn = match conn {
                    Ok(c) => c,
                    Err(_) => {
                        let _ = client.send_error(ClientError::InternalServerError(""));
                        return Err(Box::new(ClientError::InternalServerError(
                            "Internal Server Error",
                        )));
                    }
                };

                // Try to fetch the game from redis
                let game: redis::RedisResult<String> =
                    conn.get(format!("GAME:{}", join_game.game_id));
                let game = match game {
                    Ok(game) => game,
                    Err(_) => {
                        error!("No game was found");
                        let _ = client.send_error(ClientError::NoGameWasFound);
                        return Err(Box::new(ClientError::NoGameWasFound));
                    }
                };

                // Check if the game has been initialized
                // If it's not, set self as host and join game
                if game == String::new() {
                    // Serialize game object data
                    let redis_game = RedisGame {
                        shard_id: (*shard_id).to_string(),
                        host_id: client.id,
                    };
                    let serialized_redis_game = serde_json::to_string(&redis_game).unwrap();

                    // Register as host
                    let _: () =
                        conn.set(format!("GAME:{}", join_game.game_id), serialized_redis_game)?;

                    let mut client = sockets.get_mut(&client_id).unwrap();
                    client.game = Some(Game::new(
                        true,
                        join_game.game_id.clone(),
                        PartialClient::new(
                            client.id,
                            redis_game.shard_id.clone(),
                            true,
                            Some(client.socket_channel.clone()),
                        ),
                        PartialClient::new(
                            client.id,
                            redis_game.shard_id,
                            true,
                            Some(client.socket_channel.clone()),
                        ),
                        redis_pool,
                        sockets.clone(),
                    ));

                    client.send_model(DefaultModel::new(Response::new(
                        Some(JoinResponse {
                            game_id: join_game.game_id,
                            is_host: true,
                            success: true,
                        }),
                        ResponseOpCode::Join,
                    )))?;
                } else {
                    // Should join an already existing game through the shard communication protocol (redis)
                    // Or by doing it locally, if the game is hosted on the same server as the socket client
                    let redis_game: RedisGame = serde_json::from_str(&game).unwrap();

                    // Check if game is on the same server
                    if redis_game.shard_id == shard_id {
                        // Register player on this shard
                        match &mut sockets.get_mut(&redis_game.host_id) {
                            Some(game_host_client) => {
                                if let Some(game_host_client_game) = &mut game_host_client.game {
                                    // Register the local client in the host game
                                    game_host_client_game.register(PartialClient::new(
                                        client.id,
                                        shard_id.to_string(),
                                        true,
                                        Some(client.socket_channel),
                                    ));

                                    // Register the game for the client
                                    let mut client = sockets.get_mut(&client_id).unwrap();
                                    client.game = Some(Game::new(
                                        false,
                                        join_game.game_id.clone(),
                                        PartialClient::new(
                                            client.id,
                                            shard_id.to_string(),
                                            true,
                                            Some(client.socket_channel.clone()),
                                        ),
                                        PartialClient::new(
                                            redis_game.host_id,
                                            shard_id.to_string(),
                                            true,
                                            Some(game_host_client.socket_channel.clone()),
                                        ),
                                        redis_pool,
                                        sockets.clone(),
                                    ));

                                    client.send_model(DefaultModel::new(Response::new(
                                        Some(JoinResponse {
                                            game_id: join_game.game_id,
                                            is_host: false,
                                            success: true,
                                        }),
                                        ResponseOpCode::Join,
                                    )))?;
                                } else {
                                    let _: redis::RedisResult<()> = conn.del(format!("GAME:{}", join_game.game_id));
                                }
                            }
                            None => {
                                let _: redis::RedisResult<()> = conn.del(format!("GAME:{}", join_game.game_id));
                                return Err(Box::new(ClientError::ClientDoesNotExist(
                                    "Socket client does not exist",
                                )));
                            }
                        }
                    } else {
                        info!("Game is on another shard, need to register there");

                        // Serialize request
                        let request = ShardRequest::new(
                            ShardJoinRequest {
                                game_id: join_game.game_id,
                                client_id: client.id,
                                host_id: redis_game.host_id,
                                shard_id: Uuid::from_str(shard_id)?,
                            },
                            ShardRequestOpCode::Join,
                        );

                        // Send request to shard
                        sharding::send_redis(
                            &redis_pool,
                            (None, Some(Uuid::from_str(&redis_game.shard_id)?)),
                            request,
                            ShardOpCode::Request,
                        )?;
                    }
                }
            }
            RequestOpCode::Leave => {
                let mut client = sockets.get_mut(&client_id).unwrap();

                trace!("Received request to leave a game");
                if client.game.is_none() {
                    let _ = client.send_error(ClientError::NotInGame("Client was not in a game"));
                    return Err(Box::new(ClientError::NotInGame("Client was not in a game")));
                }

                // The client must be in a game at this point, it's safe to unwrap the value
                let game = client.game.take().unwrap();

                // Dropping the game object will leave the game cleanly, or shut it down if the client was host
                drop(game);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RequestOpCode {
    Join,
    Leave,
}

impl OpCodeFetcher for Request {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Request
    }
}
