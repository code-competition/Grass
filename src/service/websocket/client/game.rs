use std::{collections::HashMap, str::FromStr};

use r2d2::Pool;
use redis::Commands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    sharding::{
        self,
        communication::{
            request::{leave::ShardLeaveRequest, ShardRequest, ShardRequestOpCode},
            ShardOpCode,
        },
    },
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

use self::{
    models::event::disconnected_client::DisconnectedClientGameEvent, partial_client::PartialClient,
};

use super::{error::ClientError, SocketClient};

pub mod models;
pub mod partial_client;
pub mod redis_game;

#[derive(Debug, Clone)]
pub struct Game {
    // The client this game is on (not always host as every client (even clients to games which already have hosts) has their own Game struct)
    pub(crate) partial_client: PartialClient,

    /// If the user is host then it will have all available permissions
    pub(crate) is_host: bool,
    pub(crate) game_id: String,

    // Partial host, used for communication across clients
    pub(crate) partial_host: PartialClient,

    /// Only defined if the client is a host, this does not count the host
    pub(crate) connected_clients: Option<HashMap<Uuid, PartialClient>>,

    /// List of all sockets on this shard
    pub(crate) sockets: Sockets,

    /// Redis pool
    redis_pool: Pool<RedisConnectionManager>,

    /// If the game has been shutdown already
    shutdown: bool,
}

impl Game {
    pub fn new(
        is_host: bool,
        game_id: String,
        partial_client: PartialClient,
        partial_host: PartialClient,
        redis_pool: Pool<RedisConnectionManager>,
        sockets: Sockets,
    ) -> Game {
        let connected_clients = if is_host { Some(HashMap::new()) } else { None };
        Game {
            is_host,
            game_id,
            partial_client,
            partial_host,
            connected_clients,
            redis_pool,
            shutdown: false,
            sockets,
        }
    }

    /// Register a new client with the game
    pub fn register(&mut self, partial_client: PartialClient) {
        // Cancel if user is not game host
        if !self.is_host {
            return;
        }

        // Send existing clients to the newly connected client
        for clients in self.connected_clients.as_ref().unwrap().iter() {
            let _ = partial_client.send_message(
                DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                    game_id: self.game_id.clone(),
                    client_id: *clients.0,
                })),
                &self.redis_pool,
            );
        }

        // Send the host to the newly connected client
        let _ = partial_client.send_message(
            DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                game_id: self.game_id.clone(),
                client_id: self.partial_host.id,
            })),
            &self.redis_pool,
        );

        // Send the new client to all connected clients
        let _ = self.send_global(
            DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                game_id: self.game_id.clone(),
                client_id: partial_client.id,
            })),
            &self.redis_pool,
        );

        trace!(
            "Registering client {} in game {}",
            partial_client.id,
            self.game_id
        );
        self.connected_clients
            .as_mut()
            .unwrap()
            .insert(partial_client.id.clone(), partial_client);
    }

    /// Unregister a client from the game
    pub fn unregister(&mut self, client_id: &Uuid) {
        // Cancel if user is not game host
        if self.is_host {
            // Send the disconnected client event to all connected clients
            let _ = self.send_global(
                DefaultModel::new(GameEvent::new(DisconnectedClientGameEvent {
                    game_id: self.game_id.clone(),
                    client_id: client_id.clone(),
                })),
                &self.redis_pool,
            );

            self.connected_clients.as_mut().unwrap().remove(client_id);
        }
    }

    /// Send a message to all connected clients in the game
    pub fn send_global<'a, T>(
        &self,
        message: DefaultModel<T>,
        redis_pool: &Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize + Deserialize<'a> + Clone,
    {
        // Send message to clients
        for clients in self
            .connected_clients
            .as_ref()
            .ok_or(ClientError::InternalServerError("not host"))?
            .iter()
        {
            clients.1.send_message(message.clone(), &redis_pool)?;
        }

        // Send message to host
        self.partial_host.send_message(message, &redis_pool)?;

        Ok(())
    }

    /// Force shutdown the game
    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_host {
            return Err(Box::new(ClientError::NotGameHost(
                "Client is not game host",
            )));
        }

        trace!(
            "Host \"{}\" triggered a shutdown event",
            &self.partial_host.id
        );
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
        self.partial_host.send_message(
            DefaultModel::new(Response::new(
                Some(ShutdownResponse {
                    game_id: Some(self.game_id.clone()),
                    success: true,
                }),
                models::ResponseOpCode::Shutdown,
            )),
            &self.redis_pool,
        )?;
        Ok(())
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        if self.is_host {
            info!("Shutting down a game");
            let _ = self.shutdown();
            let mut conn = self.redis_pool.get().unwrap();
            let _: () = conn.del(format!("GAME:{}", self.game_id)).unwrap();
        } else if self.partial_host.is_local {
            if let Some(host_client) = &mut self.sockets.get_mut(&self.partial_host.id) {
                if let Some(game) = &mut host_client.game {
                    game.unregister(&self.partial_client.id);
                }
            }
        } else {
            info!("Leaving a game");
            // If the client is not host, disconnect it from the game
            let request = ShardRequest::new(
                ShardLeaveRequest {
                    game_id: self.game_id.to_string(),
                    client_id: self.partial_client.id,
                    host_id: self.partial_host.id,
                    shard_id: Uuid::from_str(&self.partial_client.shard_id).unwrap(),
                },
                ShardRequestOpCode::Leave,
            );

            // Send request to shard hosting the game
            let _ = sharding::send_redis(
                &self.redis_pool,
                (
                    None,
                    Some(Uuid::from_str(&self.partial_host.shard_id).unwrap()),
                ),
                request,
                ShardOpCode::Request,
            );
        }
    }
}
