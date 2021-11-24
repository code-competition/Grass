use std::net::SocketAddr;

use crossbeam::channel::{SendError, Sender};
use r2d2::Pool;
use redis::Commands;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::service::redis_pool::RedisConnectionManager;

use self::models::DefaultModel;

mod models;

#[derive(Debug, Clone)]
pub struct SocketClient {
    id: Uuid,
    addr: SocketAddr,
    socket_channel: Sender<Message>,
}

impl SocketClient {
    pub fn new(addr: SocketAddr, socket_channel: Sender<Message>) -> SocketClient {
        SocketClient {
            id: uuid::Uuid::new_v4(),
            addr,
            socket_channel,
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

    pub async fn on_message(
        &mut self,
        redis_pool: Pool<RedisConnectionManager>,
        message: Message,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match message {
            Message::Text(text) => {
                info!("Received text message: {}", text);
                // let mut conn = redis_pool.get()?;

                // Fetch the shard id where the client is registered
                // let shard_id: String = conn.get(format!("SOCKET:USER:{}", text))?;

                // info!("shard id: {}", shard_id);

                // Send the message to redis channel
                // let _: () = conn.publish(shard_id, text).expect("failed to publish");
            }
            Message::Binary(bin) => {
                info!("Received binary message: {:?}", bin);
            }
            Message::Ping(bin) => {
                info!("Received ping message: {:?}", bin);
                self.send(Message::Pong(bin))?
            }
            Message::Pong(bin) => {
                info!("Received pong message: {:?}", bin);
                self.send(Message::Ping(bin))?;
            }
            Message::Close(reason) => {
                info!("Received close message: {:?}", reason);
            }
        }

        Ok(())
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
