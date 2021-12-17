use futures::{future, SinkExt, StreamExt, TryStreamExt};
use r2d2::Pool;
use tokio::net::TcpStream;

use self::client::SocketClient;

use super::{redis_pool::RedisConnectionManager, Sockets};

pub mod client;

pub async fn accept_connection(
    stream: TcpStream,
    redis_pool: Pool<RedisConnectionManager>,
    sockets: Sockets,
    shard_id: String,
) {
    // Get ip address of peer
    let addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");
    trace!("Peer address: {}", addr);

    // Accept async websocket connection
    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    info!("New WebSocket connection: {}", addr);

    // Split websocket stream into two parts
    let (mut write, read) = ws_stream.split();

    // Create channel for communication from reader to writer
    let (sender, receiver) = crossbeam::channel::unbounded();

    // Register socket client
    let client = SocketClient::new(addr, sender.clone());
    sockets.insert(*client.id(), client.clone());

    // Prepare reader task
    // This task reads all incoming messages from the **CLIENT** coming through the TcpListener
    let shard_id_message = shard_id.clone();
    let read_channel = read
        .try_filter(|message| future::ready(!message.is_close()))
        .try_for_each(|message| {
            // Get the client id
            let client_id = *client.id();
            // Get the local socket from the connections hashmap
            let socket = sockets.get_mut(&client_id);

            // Trigger on_message(...) event for the client
            if let Some(mut socket) = socket {
                match futures::executor::block_on(socket.on_message(
                    redis_pool.clone(),
                    message,
                    &shard_id_message.clone(),
                    sockets.clone(),
                )) {
                    Ok(should_close) => {
                        if should_close {
                            trace!("Reached an should_close point, disconnecting socket.");
                            return future::err(
                                tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse message socket: {}", e);
                        return future::err(
                            tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                        );
                    }
                }
            }

            future::ok(())
        });

    // Handle messages sent from the SocketClient struct that was fetched from the connections hashmap
    let write_channel = tokio::spawn(async move {
        loop {
            trace!("Ready to receive message from internal write channel");
            match receiver.recv() {
                Ok(message) => match write.send(message).await {
                    Ok(_) => {
                        trace!("Sent to websocket");
                    }
                    Err(e) => {
                        error!(
                            "Socket message could not be written to websocket write channel: {}",
                            e
                        );
                    }
                },
                Err(_) => {
                    info!("No more senders are active, terminating socket");
                    break;
                }
            }
        }
    });

    // Trigger on open event for socket client
    sockets
        .get_mut(client.id())
        .expect("socket connection does not exist")
        .on_open();

    // Registers the socket in the global datastore of sockets
    let res = sockets
        .get(client.id())
        .expect("socket connection does not exist")
        .register(&redis_pool, shard_id);
    match res {
        Ok(_) => {}
        Err(e) => {
            error!(
                "An error occured while registering user on the global socket datastore: {}",
                e
            );
            sockets.remove(client.id());
        }
    }

    // Start the reader and writer tasks
    futures::pin_mut!(read_channel, write_channel);
    let _ = futures::future::select(read_channel, write_channel).await;

    // Trigger on close event
    sockets
        .get_mut(client.id())
        .expect("socket connection does not exist")
        .on_close();

    // Unregisters the socket in the global datastore of sockets
    let res = sockets
        .get(client.id())
        .expect("socket connection does not exist")
        .unregister(&redis_pool);
    match res {
        Ok(_) => {}
        Err(e) => {
            // Todo: Handle error, would be an excellent idea if the user was unregistered correctly.
            error!(
                "An error occured while unregistering user on the global socket datastore: {}",
                e
            );
        }
    }

    // Remove the socket locally
    sockets.remove(client.id());
    info!("Socket disconnected: {}", addr);
}
