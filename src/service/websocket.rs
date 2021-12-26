use std::sync::Arc;

use futures::{future, SinkExt, StreamExt, TryStreamExt};
use r2d2::Pool;
use tokio::{net::TcpStream, sync::mpsc::Sender};
use tokio_tungstenite::tungstenite::Message;

use self::client::{game::task::GameTask, SocketClient};

use super::{redis_pool::RedisConnectionManager, Sockets};

pub mod client;

type SocketSender = Sender<Message>;

pub async fn accept_connection(
    stream: TcpStream,
    available_tasks: Arc<Vec<GameTask>>,
    redis_pool: Pool<RedisConnectionManager>,
    sockets: Sockets,
    shard_id: String,
) {
    // Get ip address of peer
    let addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");

    // Accept async websocket connection
    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    info!("New WebSocket connection: {}", addr);

    // Split websocket stream into two parts
    let (mut write, read) = ws_stream.split();

    // Create channel for communication from reader to writer
    let (sender, mut receiver) = tokio::sync::mpsc::channel(200);

    // Register socket client
    let client = SocketClient::new(addr, sender.clone());
    sockets.insert(*client.id(), client.clone());

    // Prepare reader task
    // This task reads all incoming messages from the **CLIENT** coming through the TcpListener
    let local_shard_id_message = shard_id.clone();
    let local_client_id = *client.id();
    let local_redis_pool = redis_pool.clone();
    let local_available_tasks = available_tasks.clone();
    let local_sockets = sockets.clone();
    let read_channel = tokio::spawn(async move {
        let mut read_channel = read
            .try_filter(|message| future::ready(!message.is_close()))
            .enumerate();

        loop {
            match read_channel.next().await {
                Some(s) => match s.1 {
                    Ok(message) => {
                        // Trigger on_message(...) event
                        match SocketClient::on_message(
                            local_client_id,
                            local_redis_pool.clone(),
                            local_available_tasks.clone(),
                            message,
                            local_shard_id_message.clone(),
                            local_sockets.clone(),
                        )
                        .await
                        {
                            Ok(should_close) => {
                                if should_close {
                                    trace!("Reached an should_close point, disconnecting socket.");
                                    return tokio_tungstenite::tungstenite::Error::ConnectionClosed;
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse message socket: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Message reading error {}", e);
                    }
                },
                None => {
                    return tokio_tungstenite::tungstenite::Error::ConnectionClosed;
                }
            }
        }
    });

    // Handle messages sent from the SocketClient struct that was fetched from the connections hashmap
    let write_channel = tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Some(message) => match write.send(message).await {
                    Ok(_) => {
                        trace!("Sent message to websocket");
                    }
                    Err(e) => {
                        error!(
                            "Socket message could not be written to websocket write channel: {}",
                            e
                        );
                    }
                },
                None => {
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
        .on_open()
        .await;

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
