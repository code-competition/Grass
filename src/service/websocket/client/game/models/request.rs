use std::sync::Arc;

use r2d2::Pool;
use redis::Commands;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{
        game::{partial_client::PartialClient, redis_game::RedisGame, Game},
        models::{error::Error, DefaultModel, OpCode, OpCodeFetcher},
        SocketClient,
    },
    Sockets,
};

use self::{join::JoinRequest, shutdown::ShutdownRequest};

use super::{response::join::JoinResponse, Response, ResponseOpCode};

pub mod join;
pub mod shutdown;

// Models for requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    d: Option<Value>,
    op: RequestOpCode,
}

impl Request {
    pub fn handle_message(
        self,
        client: &mut SocketClient,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
        shard_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self.op {
            RequestOpCode::Join => {
                if self.d.is_none() {
                    return Err(Box::new(Error {
                        err: "Internal Server Error",
                    }));
                }

                // Check if user is already in a game
                if client.game.is_some() {
                    return Ok(());
                }

                let join_game: JoinRequest = serde_json::from_value(self.d.unwrap())?;

                // get a redis connection from the pool
                let conn = redis_pool.get();
                let mut conn = match conn {
                    Ok(c) => c,
                    Err(_) => {
                        return Err(Box::new(Error {
                            err: "Internal Server Error",
                        }));
                    }
                };

                // Try to fetch the game from redis
                let game: redis::RedisResult<String> =
                    conn.get(format!("GAME:{}", join_game.game_id.clone()));
                let game = match game {
                    Ok(game) => game,
                    Err(_) => {
                        return Err(Box::new(Error {
                            err: "No game was found",
                        }));
                    }
                };

                // Check if the game has been initialized
                if game == String::new() {
                    // Serialize game object data
                    let redis_game = RedisGame {
                        shard_id: shard_id.clone().to_string(),
                        host_id: client.id.clone(),
                    };
                    let serialized_redis_game = serde_json::to_string(&redis_game).unwrap();

                    // Register as host
                    let _: () =
                        conn.set(format!("GAME:{}", join_game.game_id), serialized_redis_game)?;

                    client.game = Some(Game::new(
                        true,
                        join_game.game_id.clone(),
                        client.id.clone(),
                        redis_game.host_id.clone(),
                        sockets.clone(),
                        redis_pool.clone(),
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
                                    game_host_client_game
                                        .register(PartialClient::new(client.id.clone(), true));
                                }
                            }
                            None => {
                                return Err(Box::new(Error {
                                    err: "Socket client does not exist",
                                }));
                            }
                        }
                    } else {
                        // TODO: Register player on another shard through pub/sub redis
                    }
                }
            }
            RequestOpCode::Shutdown => {
                if self.d.is_none() {
                    return Err(Box::new(Error {
                        err: "Internal Server Error",
                    }));
                }

                let shutdown_game: ShutdownRequest = serde_json::from_value(self.d.unwrap())?;
                if let Some(mut game) = client.game.take() {
                    if game.game_id == shutdown_game.game_id {
                        trace!("Shutting down game");
                        game.shutdown(Some(client))?;
                    } else {
                        trace!("Receieved invalid game_id from client");
                        return Err(Box::new(Error {
                            err: "Receieved invalid game_id from client",
                        }));
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RequestOpCode {
    Join,
    Shutdown,
}

impl OpCodeFetcher for Request {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Request
    }
}
