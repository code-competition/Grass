#[derive(Debug, Clone)]
pub struct Game {
    /// If the user is host then it will have all available permissions
    is_host: bool,
    game_id: String,

    /// Only defined if the client is a host, this does not count the host
    connected_clients: Option<usize>,
}

impl Game {
    pub fn new(is_host: bool, game_id: String) -> Game {
        let connected_clients = if is_host { Some(0) } else { None };
        Game {
            is_host,
            game_id,
            connected_clients,
        }
    }
}
