use std::{collections::HashMap, sync::Arc};

use dashmap::DashMap;
use r2d2::Pool;
use uuid::Uuid;

use crate::service::redis_pool::RedisConnectionManager;

use self::{
    models::{shutdown::ShutdownGameEvent, GameEvent, GameEventOpCode},
    partial_client::PartialClient,
};

use super::{models::DefaultModel, SocketClient};

pub mod models;
pub mod partial_client;
pub mod redis_game;

#[derive(Debug, Clone)]
pub struct Game {
    /// If the user is host then it will have all available permissions
    pub(crate) is_host: bool,
    pub(crate) game_id: String,

    // Host client id, used for communication across clients
    pub(crate) host_id: Uuid,

    /// Only defined if the client is a host, this does not count the host
    pub(crate) connected_clients: Option<HashMap<Uuid, PartialClient>>,

    /// List of all connected sockets
    sockets: Arc<DashMap<Uuid, SocketClient>>,

    /// Redis pool
    redis_pool: Pool<RedisConnectionManager>,
}

impl Game {
    pub fn new(
        is_host: bool,
        game_id: String,
        host_id: Uuid,
        sockets: Arc<DashMap<Uuid, SocketClient>>,
        redis_pool: Pool<RedisConnectionManager>,
    ) -> Game {
        let connected_clients = if is_host { Some(HashMap::new()) } else { None };
        Game {
            is_host,
            game_id,
            host_id,
            connected_clients,
            sockets,
            redis_pool,
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

    /// Force shutdown the game
    pub fn shutdown(&self) {
        // Send quit message to all connected clients or only to the host
        if !self.is_host {
            return;
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
    }
}
