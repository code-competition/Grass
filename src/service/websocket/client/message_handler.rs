use std::f32::consts::E;

use r2d2::Pool;
use serde_json::Value;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::models::{Error, JoinGame},
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
                warn!("{:?}", join_game);
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
