use std::collections::HashMap;

use uuid::Uuid;

use self::partial_client::PartialClient;

pub mod partial_client;
pub mod redis_game;

#[derive(Debug, Clone)]
pub struct Game {
    /// If the user is host then it will have all available permissions
    is_host: bool,
    game_id: String,

    // Host client id, used for communication across clients
    host_id: Uuid,

    /// Only defined if the client is a host, this does not count the host
    connected_clients: Option<HashMap<Uuid, PartialClient>>,
}

impl Game {
    pub fn new(is_host: bool, game_id: String, host_id: Uuid) -> Game {
        let connected_clients = if is_host { Some(HashMap::new()) } else { None };
        Game {
            is_host,
            game_id,
            host_id,
            connected_clients,
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
    pub fn shutdown(&mut self) {
        // Send quit message to all connected clients or only to the host
    }
}
