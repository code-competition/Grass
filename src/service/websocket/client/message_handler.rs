use std::sync::Arc;

use dashmap::DashMap;
use r2d2::Pool;
use redis::Commands;
use serde_json::Value;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::game::{partial_client::PartialClient, Game},
};

use super::{
    game::redis_game::RedisGame,
    models::{
        error::Error, join_game::JoinGame, join_game_response::JoinGameResponse, DefaultModel,
        OpCode, shutdown_game::ShutdownGame,
    },
    SocketClient,
};

pub struct ClientMessageHandler {}

impl ClientMessageHandler {
    pub fn handle_message(
        client: &mut SocketClient,
        sockets: Arc<DashMap<Uuid, SocketClient>>,
        redis_pool: Pool<RedisConnectionManager>,
        model: DefaultModel<Value>,
        shard_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = if let Some(data) = model.d {
            data
        } else {
            return Err(Box::new(Error {
                err: "No data was sent with opcode",
            }));
        };

        trace!("Receieved message from client, parsing it OpCode: {:?}", &model.op);

        match model.op {
            OpCode::JoinGame => {
                let join_game: JoinGame = serde_json::from_value(data)?;

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
                        sockets.clone(),
                        redis_pool.clone(),
                    ));
                    client.send_model(DefaultModel::new(JoinGameResponse {
                        game_id: join_game.game_id,
                        is_host: true,
                        success: true,
                    }))?;
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
            },
            OpCode::ShutdownGame => {
                let shutdown_game: ShutdownGame = serde_json::from_value(data)?;
                if let Some(game) = &client.game {
                    if game.game_id == shutdown_game.game_id {
                        trace!("Shutting down game");
                        game.shutdown();
                    }

                    trace!("Receieved invalid game_id from client");
                }
                // Todo: unregister from redis
                client.game = None;
            },
            _ => {
                return Err(Box::new(Error {
                    err: "Invalid receieve opcode",
                }));
            }
        }

        Ok(())
    }
}
