use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PartialClient {
    pub(crate) id: Uuid,

    /// true if the player is local to the server\
    /// false if the player is on another shard
    pub(crate) is_local: bool,
}

