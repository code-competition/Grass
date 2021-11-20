use std::sync::Arc;

use dashmap::DashMap;
use futures::channel::oneshot::{Receiver, Sender};
use r2d2::Pool;
use redis::{ControlFlow, PubSubCommands};
use tokio::net::TcpListener;
use uuid::Uuid;

use self::{
    error::CriticalError, redis_pool::RedisConnectionManager, websocket::client::SocketClient,
};

pub mod error;
pub mod extended_select;
pub mod redis_pool;
pub mod websocket;

pub struct Service<'a> {
    // shard enviromental variables
    shard_id: &'a str,
    host_addr: &'a str,
    redis_addr: &'a str,

    // List of open socket connections
    connections: Arc<DashMap<Uuid, SocketClient>>,

    // Redis connection pool
    redis_pool: Pool<RedisConnectionManager>,

    // Error channel to trigger shutdown of service if something goes wrong
    error_channel: (
        Option<Sender<CriticalError>>,
        Option<Receiver<CriticalError>>,
    ),
}

impl<'a> Service<'a> {
    pub async fn new(shard_id: &'a str, host_addr: &'a str, redis_addr: &'a str) -> Service<'a> {
        // Create redis connection poool
        let manager = RedisConnectionManager::new(redis_addr).unwrap();
        let redis_pool = r2d2::Pool::builder().build(manager).unwrap();

        // Initialize thread channel to handle critical errors that may occur inside the application
        let (error_tx, error_rx) = futures::channel::oneshot::channel::<CriticalError>();

        Self {
            shard_id,
            host_addr,
            redis_addr,

            connections: Arc::new(DashMap::new()),
            redis_pool,

            error_channel: (Some(error_tx), Some(error_rx)),
        }
    }

    pub async fn run<T>(
        &mut self,
        middleware: fn(Arc<DashMap<Uuid, SocketClient>>, T),
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: redis::FromRedisValue + 'static,
    {
        // Create a seperate thread for WebSockets
        let redis_pool = self.redis_pool.clone();
        let socket_connections = self.connections.clone();
        let shard_id = self.shard_id.to_string();
        let host_addr = self.host_addr.to_string();
        let jh_ws = tokio::spawn(async move {
            trace!("Launching socket shard");
            let try_socket = TcpListener::bind(&host_addr).await;
            let listener = try_socket.expect("Failed to bind");
            info!("Socket shard listening on: {}", host_addr);

            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(websocket::accept_connection(
                    stream,
                    redis_pool.clone(),
                    socket_connections.clone(),
                    shard_id.to_string(),
                ));
            }
        });

        // Create a seperate thread for PubSub channels
        let shard_id = self.shard_id.to_string();
        let redis_addr = self.redis_addr.to_string();
        let socket_connections = self.connections.clone();
        let jh_presence = tokio::spawn(async move {
            let client = redis::Client::open(redis_addr).expect("redis connection failed");
            let mut con = client
                .get_connection()
                .expect("could not get redis connection");

            info!("shard id registered to pub/sub: {}", shard_id);

            // Subscribe to presence channel and receive messages from other shards
            let _: () = con
                .subscribe(&[shard_id], |msg| {
                    let payload: T = msg
                        .get_payload()
                        .expect("could not get pub/sub message payload");

                    // Call middleware function and pass in the payload
                    middleware(socket_connections.clone(), payload);

                    ControlFlow::Continue
                })
                .unwrap();
        });

        // This custom select function exits when one of the futures returns,
        // In case of a critical error the service will exit (error_rx)
        // and the option it returns will contain the CriticalError
        let critical_error =
            extended_select::select(jh_ws, jh_presence, self.error_channel.1.take().unwrap()).await;
        match critical_error {
            Some(error) => {
                match error {
                    Ok(error) => {
                        error!("Critical error: {}", error);
                        std::process::exit(error.get_code());
                    }

                    // Something is very wrong if this happens, the future should not be canceled, ever.
                    Err(e) => Err(e.into()),
                }
            }

            // Normal service shutdown
            None => Ok(()),
        }
    }
}