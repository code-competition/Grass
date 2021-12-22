use std::{net::SocketAddr, sync::Arc};

use crossbeam::channel::{SendError, Sender};
use r2d2::Pool;
use redis::Commands;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::service::{redis_pool::RedisConnectionManager, Sockets};

use self::{
    error::ClientError,
    game::{task::GameTask, Game},
    models::{hello::Hello, DefaultModel},
};
use message_handler::ClientMessageHandler;

pub mod error;
pub mod game;
pub mod message_handler;
pub mod models;

#[derive(Debug, Clone)]
pub struct SocketClient {
    pub(crate) id: Uuid,
    addr: SocketAddr,
    pub(crate) socket_channel: Sender<Message>,

    // Some(...) if user is in a game
    pub(crate) game: Option<Game>,
}

impl SocketClient {
    pub fn new(addr: SocketAddr, socket_channel: Sender<Message>) -> SocketClient {
        SocketClient {
            id: uuid::Uuid::new_v4(),
            addr,
            socket_channel,
            game: None,
        }
    }

    /// Triggered once the client has been registered and is connected
    ///
    /// Sends a hello with the socket id
    pub fn on_open(&mut self) {
        trace!("Client connected with address {}", self.addr);

        let model = DefaultModel::new(Hello { id: self.id });
        self.send_model(model)
            .expect("could not send hello message");
    }

    /// Triggered when connection is closing
    pub fn on_close(&mut self) {
        if let Some(game) = self.game.take() {
            drop(game);
        }
    }

    /// Registers the socket client in the global connection datastore
    pub fn register(
        &self,
        redis_pool: &Pool<RedisConnectionManager>,
        shard_id: String,
    ) -> Result<(), String> {
        let mut conn = redis_pool.get().map_err(|e| e.to_string())?;
        let _: () = conn
            .set(format!("SOCKET:USER:{}", self.id), shard_id)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Unregisters the socket client from the global connection datastore
    pub fn unregister(&self, redis_pool: &Pool<RedisConnectionManager>) -> Result<(), String> {
        let mut conn = redis_pool.get().map_err(|e| e.to_string())?;
        let _ = conn
            .del(format!("SOCKET:USER:{}", self.id))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Called when a client receives a new message
    pub async fn on_message(
        client_id: Uuid,
        redis_pool: Pool<RedisConnectionManager>,
        available_tasks: Arc<Vec<GameTask>>,
        message: Message,
        shard_id: &str,
        sockets: Sockets,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let mut should_close = false;
        match message {
            Message::Text(text) => {
                info!("Received text message: {}", text);
                // Try to parse the message according to the Default JSON layout
                let model: Result<DefaultModel<Value>, serde_json::Error> =
                    serde_json::from_str(&text);
                match model {
                    Ok(model) => {
                        if let Err(e) = ClientMessageHandler::handle_message(
                            client_id,
                            sockets,
                            redis_pool,
                            available_tasks,
                            model,
                            shard_id,
                        ).await {
                            error!("Error while handling message {}", e);
                            should_close = true;
                        }
                    }
                    Err(e) => {
                        error!("Client reached an error {:?}", e);
                        error!("Receieved invalid model from socket, closing connection.");
                        should_close = true;
                        sockets.get(&client_id).unwrap().send_error(
                            ClientError::InvalidMessage("Invalid model, closing connection."),
                        )?;
                    }
                }
            }
            Message::Ping(bin) => sockets.get(&client_id).unwrap().send(Message::Pong(bin))?,
            Message::Pong(bin) => {
                sockets.get(&client_id).unwrap().send(Message::Ping(bin))?;
            }
            Message::Close(reason) => {
                info!("Received close message: {:?}", reason);
                return Ok(true);
            }
            _ => {}
        }

        Ok(should_close)
    }

    /// Sends a error to the client
    #[inline]
    pub fn send_error(&self, err: ClientError) -> Result<(), SendError<Message>> {
        self.send_model(DefaultModel::new(err))
    }

    /// Sends a model (JSON serializable object) to the client
    #[inline]
    pub fn send_model<'a, T>(&self, default: DefaultModel<T>) -> Result<(), SendError<Message>>
    where
        T: serde::Serialize + serde::Deserialize<'a>,
    {
        self.send(Message::Text(serde_json::to_string(&default).unwrap()))
    }

    /// Sends a raw websocket message
    #[inline]
    pub fn send(&self, message: Message) -> Result<(), SendError<Message>> {
        self.socket_channel().send(message)
    }

    /// Get a reference to the socket client's id.
    #[inline]
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a reference to the socket client's socket channel.
    #[inline]
    pub fn socket_channel(&self) -> &Sender<Message> {
        &self.socket_channel
    }
}
