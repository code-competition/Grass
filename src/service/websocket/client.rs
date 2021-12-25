use std::{net::SocketAddr, sync::Arc};

use r2d2::Pool;
use redis::Commands;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::game::models::{
        response::timeout::TimeoutResponse, Response, ResponseOpCode,
    },
    Sockets,
};

use self::{
    error::ClientError,
    game::{task::GameTask, Game},
    models::{hello::Hello, DefaultModel},
};
use message_handler::ClientMessageHandler;

use super::SocketSender;

pub mod error;
pub mod game;
pub mod message_handler;
pub mod models;

#[derive(Debug, Clone)]
pub struct SocketClient {
    pub(crate) id: Uuid,
    addr: SocketAddr,
    pub(crate) send_channel: SocketSender,

    // Some(...) if user is in a game
    pub(crate) game: Option<Game>,
}

impl SocketClient {
    pub fn new(addr: SocketAddr, send_channel: SocketSender) -> SocketClient {
        SocketClient {
            id: uuid::Uuid::new_v4(),
            addr,
            send_channel,
            game: None,
        }
    }

    /// Triggered once the client has been registered and is connected
    ///
    /// Sends a hello with the socket id
    pub async fn on_open(&mut self) {
        trace!("Client connected with address {}", self.addr);

        let model = DefaultModel::new(Hello { id: self.id });
        self.send_model(model)
            .await
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
    pub async fn on_message<'a>(
        client_id: Uuid,
        redis_pool: Pool<RedisConnectionManager>,
        available_tasks: Arc<Vec<GameTask>>,
        message: Message,
        shard_id: String,
        sockets: Sockets,
    ) -> Result<bool, ClientError<'a>> {
        let mut should_close = false;
        trace!("Parsing socket message");
        match message {
            Message::Text(text) => {
                info!("Received text message: {}", text);
                // Try to parse the message according to the Default JSON layout
                let model: Result<DefaultModel<Value>, serde_json::Error> =
                    serde_json::from_str(&text);
                match model {
                    Ok(model) => {
                        trace!("calling handle_message");
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            ClientMessageHandler::handle_message(
                                client_id,
                                &sockets,
                                redis_pool,
                                available_tasks,
                                &model,
                                &shard_id,
                            ),
                        )
                        .await
                        {
                            Ok(res) => {
                                if let Err(e) = res {
                                    error!("Error while handling message {}", e);
                                    should_close = true;
                                }
                            }
                            Err(e) => {
                                let client = sockets.get(&client_id).unwrap().clone();
                                client
                                    .send_model(DefaultModel::new(Response::new(
                                        Some(TimeoutResponse::new(model)),
                                        ResponseOpCode::Timeout,
                                    )))
                                    .await
                                    .map_err(|_| ClientError::SendError)?;
                                trace!("handle_message timed out, {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Client reached an error {:?}", e);
                        error!("Receieved invalid model from socket, closing connection.");
                        should_close = true;
                        sockets
                            .get(&client_id)
                            .unwrap()
                            .send_error(ClientError::InvalidMessage(
                                "Invalid model, closing connection.",
                            ))
                            .await
                            .map_err(|_| ClientError::SendError)?;
                    }
                }
            }
            Message::Ping(bin) => sockets
                .get(&client_id)
                .unwrap()
                .send(Message::Pong(bin))
                .await
                .map_err(|_| ClientError::SendError)?,
            Message::Pong(bin) => {
                sockets
                    .get(&client_id)
                    .unwrap()
                    .send(Message::Ping(bin))
                    .await
                    .map_err(|_| ClientError::SendError)?;
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
    pub async fn send_error(&self, err: ClientError<'_>) -> Result<(), ClientError<'_>> {
        error!("Sending error {} to client", err);
        self.send_model(DefaultModel::new(err)).await
    }

    /// Sends a model (JSON serializable object) to the client
    #[inline]
    pub async fn send_model<'a, T>(&self, default: DefaultModel<T>) -> Result<(), ClientError<'_>>
    where
        T: serde::Serialize + serde::Deserialize<'a>,
    {
        self.send(Message::Text(serde_json::to_string(&default).unwrap()))
            .await
    }

    /// Sends a raw websocket message
    #[inline]
    pub async fn send(&self, message: Message) -> Result<(), ClientError<'_>> {
        self.send_channel
            .send(message)
            .await
            .map_err(|_| ClientError::SendError)
    }

    /// Get a reference to the socket client's id.
    #[inline]
    pub fn id(&self) -> &Uuid {
        &self.id
    }
}
