use std::collections::HashMap;

use r2d2::Pool;
use redis::Commands;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{
        game::models::{
            event::shutdown::ShutdownGameEvent, response::shutdown::ShutdownResponse, GameEvent,
            GameEventOpCode, Response,
        },
        models::DefaultModel,
    },
    Sockets,
};

use self::partial_client::PartialClient;

use super::SocketClient;

pub mod models;
pub mod partial_client;
pub mod redis_game;

#[derive(Debug, Clone)]
pub struct Game {
    // The client this game is on (not always host as every client (even clients to games which already have hosts) has their own Game struct)
    pub(crate) client_id: Uuid,

    /// If the user is host then it will have all available permissions
    pub(crate) is_host: bool,
    pub(crate) game_id: String,

    // Host client id, used for communication across clients
    pub(crate) host_id: Uuid,

    /// Only defined if the client is a host, this does not count the host
    pub(crate) connected_clients: Option<HashMap<Uuid, PartialClient>>,

    /// List of all connected sockets
    sockets: Sockets,

    /// Redis pool
    redis_pool: Pool<RedisConnectionManager>,

    /// If the game has been shutdown already
    shutdown: bool,
}

impl Game {
    pub fn new(
        is_host: bool,
        game_id: String,
        client_id: Uuid,
        host_id: Uuid,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
    ) -> Game {
        let connected_clients = if is_host { Some(HashMap::new()) } else { None };
        Game {
            is_host,
            game_id,
            client_id,
            host_id,
            connected_clients,
            sockets,
            redis_pool,
            shutdown: false,
        }
    }

    /// Register a new client with the game
    pub fn register(&mut self, partial_client: PartialClient) {
        // Cancel if user is not game host
        if let Some(connected_clients) = &mut self.connected_clients {
            debug!("Register client {:?}", (&partial_client));
            connected_clients.insert(partial_client.id.clone(), partial_client);
        }
    }

    /// Unregister a client from the game
    pub fn unregister(&mut self, client_id: &Uuid) {
        // Cancel if user is not game host
        if let Some(connected_clients) = &mut self.connected_clients {
            connected_clients.remove(client_id);
        }
    }

    /// Force shutdown the game
    pub fn shutdown(
        &mut self,
        client: Option<&SocketClient>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Send quit message to all connected clients or only to the host
        if !self.is_host {
            if let Some(client) = client {
                client.send_model(DefaultModel::new(Response::new(
                    Some(ShutdownResponse {
                        game_id: None,
                        success: false,
                    }),
                    models::ResponseOpCode::Shutdown,
                )))?;
            }

            return Ok(());
        }

        trace!("Host \"{}\" triggered a shutdown event", &self.host_id);
        for client in self.connected_clients.as_ref().unwrap().iter() {
            let partial = client.1;
            let res = partial.send_message(
                DefaultModel::new(GameEvent::new(
                    Some(ShutdownGameEvent {
                        game_id: self.game_id.clone(),
                    }),
                    GameEventOpCode::Shutdown,
                )),
                Some(&self.sockets),
                &self.redis_pool,
            );

            match res {
                Ok(_) => (),
                Err(e) => {
                    error!("Critical error when ending game {}", e);
                }
            }
        }

        self.connected_clients.as_mut().unwrap().clear();
        self.shutdown = true;

        // Send final goodbye to the host
        if let Some(client) = client {
            client.send_model(DefaultModel::new(Response::new(
                Some(ShutdownResponse {
                    game_id: Some(self.game_id.clone()),
                    success: true,
                }),
                models::ResponseOpCode::Shutdown,
            )))?;
        }

        Ok(())
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        let mut conn = self
            .redis_pool
            .get()
            .expect("failed to get redis connection");
        let _: () = conn
            .del(format!("GAME:{}", self.game_id))
            .expect("could not remove game from redis");

        if !self.shutdown {
            let _ = self.shutdown(None);
        }
    }
}
