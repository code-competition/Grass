use std::collections::HashMap;

use r2d2::Pool;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{
        game::models::{
            event::{connected_client::ConnectedClientGameEvent, shutdown::ShutdownGameEvent},
            response::shutdown::ShutdownResponse,
            GameEvent, Response,
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

    // Partial host, used for communication across clients
    pub(crate) partial_host: PartialClient,

    /// Only defined if the client is a host, this does not count the host
    pub(crate) connected_clients: Option<HashMap<Uuid, PartialClient>>,

    /// List of all connected sockets
    /// ! Extreme warning for deadlocks, shouldn't be used!
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
        partial_host: PartialClient,
        sockets: Sockets,
        redis_pool: Pool<RedisConnectionManager>,
    ) -> Game {
        let connected_clients = if is_host { Some(HashMap::new()) } else { None };
        Game {
            is_host,
            game_id,
            client_id,
            partial_host,
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
            trace!("Registering client {} in game", partial_client.id);
            for clients in connected_clients.iter() {
                // Send the new client to every old client already connected
                let _ = clients.1.send_message(
                    DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                        game_id: self.game_id.clone(),
                        client_id: partial_client.id,
                    })),
                    &self.redis_pool,
                );

                // Send existing client to the newly connected client
                let _ = partial_client.send_message(
                    DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                        game_id: self.game_id.clone(),
                        client_id: *clients.0,
                    })),
                    &self.redis_pool,
                );
            }

            // Send the new client to the host
            let _ = self.partial_host.send_message(
                DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                    game_id: self.game_id.clone(),
                    client_id: partial_client.id,
                })),
                &self.redis_pool,
            );

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

        trace!("Host \"{}\" triggered a shutdown event", &self.partial_host.id);
        for client in self.connected_clients.as_ref().unwrap().iter() {
            let partial = client.1;
            let res = partial.send_message(
                DefaultModel::new(GameEvent::new(ShutdownGameEvent {
                    game_id: self.game_id.clone(),
                })),
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
        // Todo: either shutdown the game or leave the game
    }
}
