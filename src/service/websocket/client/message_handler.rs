use r2d2::Pool;
use redis::Commands;
use serde_json::Value;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{models::{Error, JoinGame}, game::Game},
};

use super::{
    models::{DefaultModel, OpCode},
    SocketClient,
};

pub struct ClientMessageHandler {}

impl ClientMessageHandler {
    pub fn handle_message(
        client: &mut SocketClient,
        redis_pool: Pool<RedisConnectionManager>,
        model: DefaultModel<Value>,
        shard_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = if let Some(data) = model.d {
            data
        } else {
            return Err(Box::new(Error {
                err: "No data was sent with opcode",
                code: 82,
            }));
        };

        match model.op {
            OpCode::JoinGame => {
                let join_game: JoinGame = serde_json::from_value(data)?;

                let conn = redis_pool.get();
                let mut conn = match conn {
                    Ok(c) => c,
                    Err(_) => {
                        return Err(Box::new(Error {
                            err: "Internal Server Error",
                            code: 100,
                        }));
                    },
                };

                let game: redis::RedisResult<String> = conn.get(format!("GAME:{}", join_game.game_id));
                let game = match game {
                    Ok(game) => game,
                    Err(_) => {
                        return Err(Box::new(Error {
                            err: "No game was found",
                            code: 100,
                        }));
                    },
                };

                // Check if the game has been initialized
                if game == String::new() {
                    // Register as host
                    let _: () = conn.set(format!("GAME:{}", join_game.game_id), format!("SHARD_ID:{}", shard_id))?;
                    client.game = Some(Game::new(true, join_game.game_id));
                } else {
                    // Should join an already existing game through the shard communication protocol (redis)
                    // Or by doing it locally, if the game is hosted on the same server as the socket client
                }
            }
            _ => {
                return Err(Box::new(Error {
                    err: "Invalid receieve opcode",
                    code: 83,
                }));
            }
        }

        // Returning true will close the connection and returning false will keep it open
        Ok(())
    }
}
