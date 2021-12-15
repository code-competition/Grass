use std::net::SocketAddr;

use crossbeam::channel::{SendError, Sender};
use r2d2::Pool;
use redis::Commands;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::service::redis_pool::RedisConnectionManager;

use self::{
    game::Game,
    models::{DefaultModel, Error},
};
use message_handler::ClientMessageHandler;

mod game;
mod message_handler;
mod models;

#[derive(Debug, Clone)]
pub struct SocketClient {
    id: Uuid,
    addr: SocketAddr,
    socket_channel: Sender<Message>,

    // Some(...) if user is in a game
    game: Option<Game>,
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
        let model = DefaultModel::new(models::Hello { id: self.id });
        self.send_model(model)
            .expect("could not send hello message");
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
        &mut self,
        redis_pool: Pool<RedisConnectionManager>,
        message: Message,
        shard_id: &str,
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
                        if let Err(_) =
                            ClientMessageHandler::handle_message(self, redis_pool, model, shard_id)
                        {
                            should_close = true;
                        }
                    }
                    Err(e) => {
                        error!("Client reached an error {:?}", e);
                        error!("Receieved invalid model from socket, closing connection.");
                        should_close = true;
                        self.send_error("Invalid model, closing connection.", 0x1)?;
                    }
                }
            }
            Message::Ping(bin) => self.send(Message::Pong(bin))?,
            Message::Pong(bin) => {
                self.send(Message::Ping(bin))?;
            }
            Message::Close(reason) => {
                info!("Received close message: {:?}", reason);
                return Ok(true);
            }
            _ => {}
        }

        Ok(should_close)
    }

    pub fn send_error<'a>(&self, err: &'a str, code: u32) -> Result<(), SendError<Message>> {
        self.send_model(DefaultModel::new(Error { err, code }))
    }

    /// Sends a model (JSON serializable object) to the client
    #[inline]
    pub fn send_model<'a, T>(&self, default: DefaultModel<T>) -> Result<(), SendError<Message>>
    where
        T: serde::Serialize + serde::Deserialize<'a>,
    {
        self.send(Message::Text(serde_json::to_string(&default).unwrap()))
    }

    /// Sends a raw tungstenite socket message
    #[inline]
    pub fn send(&self, message: Message) -> Result<(), SendError<Message>> {
        info!("not triggered?");
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
