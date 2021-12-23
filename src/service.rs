use std::{path::Path, sync::Arc, collections::HashMap};

use futures::{
    channel::oneshot::{Receiver, Sender},
    Future,
};
use r2d2::Pool;
use redis::{ControlFlow, PubSubCommands};
use tokio::{net::TcpListener, sync::RwLock};
use uuid::Uuid;

use self::{
    error::CriticalError,
    redis_pool::RedisConnectionManager,
    sharding::communication::ShardDefaultModel,
    websocket::client::{game::task::GameTask, SocketClient},
};

pub mod error;
pub mod extended_select;
pub mod redis_pool;
pub mod sharding;
pub mod task_loader;
pub mod websocket;

pub type Sockets = Arc<RwLock<HashMap<Uuid, SocketClient>>>;

/*
    # Flow chart of the websocket part of this service

    main() ────────────────► service.run(...) ──┐
                                                │
    ┌───────── on connection received ──────────┘
    │
    └────────► accept_connection(...) ───► split reader and writer
                                                     │
                                                     │
    ┌───────── register user in database ◄───────────┘
    │
    └────► on received from other sharding ────► send to middleware
*/

type ShardingMiddleware<F> =
    fn(String, Sockets, Pool<RedisConnectionManager>, ShardDefaultModel) -> F;

pub struct MiddlewareManager<F>
where
    F: Future,
{
    function: ShardingMiddleware<F>,
}

impl<F> MiddlewareManager<F>
where
    F: Future,
{
    pub fn new(fun: ShardingMiddleware<F>) -> Self {
        Self { function: fun }
    }
}

impl<F> Clone for MiddlewareManager<F>
where
    F: Future,
{
    fn clone(&self) -> Self {
        MiddlewareManager {
            function: self.function,
        }
    }
}

pub struct Service<'a> {
    // shard enviromental variables
    shard_id: &'a str,
    host_addr: &'a str,
    redis_addr: &'a str,

    // List of open socket connections
    connections: Sockets,

    // Redis connection pool
    redis_pool: Pool<RedisConnectionManager>,

    // Available tasks
    available_tasks: Arc<Vec<GameTask>>,

    // Error channel to trigger shutdown of service if something goes wrong
    error_channel: (
        Option<Sender<CriticalError>>,
        Option<Receiver<CriticalError>>,
    ),
}

impl<'a> Service<'a> {
    pub async fn new(
        shard_id: &'a str,
        host_addr: &'a str,
        game_loading_path: &Path,
        redis_addr: &'a str,
    ) -> Service<'a> {
        // Create redis connection poool
        let manager = RedisConnectionManager::new(redis_addr).unwrap();
        let redis_pool = r2d2::Pool::builder().build(manager).unwrap();

        // Initialize thread channel to handle critical errors that may occur inside the application
        let (error_tx, error_rx) = futures::channel::oneshot::channel::<CriticalError>();

        // Load available tasks
        let tasks = task_loader::load_tasks(
            &std::fs::read_to_string(game_loading_path).expect("file not found"),
        );

        Self {
            shard_id,
            host_addr,
            redis_addr,

            connections: Arc::new(RwLock::new(HashMap::new())),
            redis_pool,

            available_tasks: Arc::new(tasks),

            error_channel: (Some(error_tx), Some(error_rx)),
        }
    }

    pub async fn run<F>(
        &mut self,
        _middleware: MiddlewareManager<F>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Future + Send + 'static,
    {
        // Create a seperate thread for WebSockets
        let redis_pool = self.redis_pool.clone();
        let socket_connections = self.connections.clone();
        let shard_id = self.shard_id.to_string();
        let host_addr = self.host_addr.to_string();

        // Spawns websocket server task which handles all incoming tokio-tungstenite connections
        // And redirects them into the function websocket::accept_connection(...)
        // TCP system works with tokio-tungstenite through tokios TcpListener
        let pool = redis_pool.clone();
        let available_tasks = self.available_tasks.clone();
        let joinhandle_ws = tokio::spawn(async move {
            trace!("Launching socket shard");
            let try_socket = TcpListener::bind(&host_addr).await;
            let listener = try_socket.expect("Failed to bind");
            info!("Socket shard listening on: {}", host_addr);

            while let Ok((stream, _)) = listener.accept().await {
                let available_tasks = available_tasks.clone();
                tokio::spawn(websocket::accept_connection(
                    stream,
                    available_tasks,
                    pool.clone(),
                    socket_connections.clone(),
                    shard_id.to_string(),
                ));
            }
        });

        // Clone arcs and get ip addresses for redis to send into redis reader task
        let shard_id = self.shard_id.to_string();
        let redis_addr = self.redis_addr.to_string();
        let _socket_connections = self.connections.clone();

        // Create a seperate thread for PubSub channels
        // Spawns the tokio task that handles all incoming messages from redis
        let _pool = redis_pool.clone();
        let joinhandle_presence = tokio::spawn(async move {
            // Opens a new redis connection outside the pool to leverage better connection speeds
            let client = redis::Client::open(redis_addr).expect("redis connection failed");
            let mut con = client
                .get_connection()
                .expect("could not get redis connection");

            info!("shard id registered to pub/sub: {}", shard_id);

            // Subscribe to presence channel and receive messages from other sharding (socket servers)
            let _local_shard_id = shard_id.clone();
            let _: () = con
                .subscribe(&[shard_id], |_msg| {
                    // trace!("Receiving message from shard");
                    // let local_middleware = middleware.clone();
                    // let local_socket_connections = socket_connections.clone();
                    // let local_pool = pool.clone();
                    // let local_shard_id = local_shard_id.clone();
                    // trace!("Parsing payload from shard message");
                    // let payload: Vec<u8> = msg
                    //     .get_payload()
                    //     .expect("could not get pub/sub message payload");
                    // trace!(
                    //     "Payload from shard message has length {} bytes",
                    //     payload.len()
                    // );

                    // // Todo: error handling
                    // let payload = flexbuffers::Reader::get_root(payload.as_slice()).unwrap();
                    // let model = ShardDefaultModel::deserialize(payload).unwrap();
                    // trace!("Deserialized payload and found opcode: {:?}", &model.op);

                    // // Call middleware function and pass in the payload
                    // futures::executor::block_on((local_middleware.function)(
                    //     local_shard_id,
                    //     local_socket_connections,
                    //     local_pool,
                    //     model,
                    // ));

                    ControlFlow::Continue
                })
                .unwrap();
        });

        // This custom select function exits when one of the futures returns,
        // In case of a critical error the service will exit (error_rx)
        // and the option it returns will contain the CriticalError
        let critical_error = extended_select::select(
            joinhandle_ws,
            joinhandle_presence,
            self.error_channel.1.take().unwrap(),
        )
        .await;
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
