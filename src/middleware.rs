use crate::service::{shards::communication::ShardDefaultModel, Sockets};

/// shard_payload_interceptor
///
/// Intercepts messages from other shards and handles them
pub fn shard_payload_interceptor(_connections: Sockets, payload: ShardDefaultModel) {
    info!("Received payload: {:?}", payload);
}
