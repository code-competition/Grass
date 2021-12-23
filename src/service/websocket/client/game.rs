use std::{collections::HashMap, str::FromStr, sync::Arc};

use r2d2::Pool;
use rand::prelude::SliceRandom;
use redis::Commands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::{
    error::ServiceError,
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
            response::{leave::LeaveResponse, shutdown::ShutdownResponse},
            GameEvent, Response, ResponseOpCode,
        },
        models::DefaultModel,
    },
    Sockets,
};

use self::{
    models::event::{
        disconnected_client::DisconnectedClientGameEvent, start::StartGameEvent,
        task::TaskGameEvent,
    },
    partial_client::PartialClient,
    sandbox::{sandbox_service_client::SandboxServiceClient, SandboxRequest},
    task::GameTask,
};

use super::error::ClientError;

pub mod models;
pub mod partial_client;
pub mod redis_game;
pub mod task;

pub mod sandbox {
    tonic::include_proto!("sandbox");
}

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

    /// If the game is started, competition is active
    is_started: bool,

    /// If the game is open for registration
    public: bool,

    /// List of all tasks to finish before the game ends
    tasks: Vec<GameTask>,
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
            is_started: false,
            sockets,
            public: true,
            tasks: Vec::new(),
        }
    }

    /// Starts the game for all clients
    pub fn start(
        &mut self,
        available_tasks: Arc<Vec<GameTask>>,
        task_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.public = false;

        // Choose a random programming question
        self.tasks = available_tasks
            .choose_multiple(&mut rand::thread_rng(), task_count)
            .map(|x| x.to_owned())
            .collect();

        // Verify that the tasks have been filled up
        if self.tasks.len() != task_count {
            return Err(Box::new(ServiceError::InternalServerError));
        }

        // Get the first task to send to all clients
        let task = self.get_task_indexed(0).unwrap();

        // Send game start notice to all clients
        let _ = self.send_global(
            DefaultModel::new(GameEvent::new(StartGameEvent { task_count })),
            &self.redis_pool,
        );

        // Send the first task to all clients
        let _ = self.send_global(
            DefaultModel::new(GameEvent::new(TaskGameEvent {
                task: task.to_owned(),
            })),
            &self.redis_pool,
        );

        Ok(())
    }

    /// Compile client code and return result
    pub async fn compile_code(
        &mut self,
        client_id: &Uuid,
        code: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = SandboxServiceClient::connect("http://127.0.0.1:50051").await?;

        let request = tonic::Request::new(SandboxRequest {
            user_id: client_id.to_string(),
            code,
            language: sandbox::Language::Rust as i32,
        });

        let response = client.compile(request).await?;

        println!("{:?}", (response));

        Ok(())
    }

    /// Fetches a task at index
    pub fn get_task_indexed(&self, index: usize) -> Result<&GameTask, Option<()>> {
        if !self.is_started {
            Ok(self.tasks.get(index).ok_or(Some(()))?)
        } else {
            Err(None)
        }
    }

    /// Register a new client with the game
    pub fn register(&mut self, partial_client: PartialClient) -> Result<(), ()> {
        trace!("Registering client from game");
        // Cancel if user is not game host
        if !self.is_host || !self.public {
            return Err(());
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
            .insert(partial_client.id, partial_client);

        Ok(())
    }

    /// Unregister a client from the game
    pub fn unregister(&mut self, client_id: &Uuid) {
        trace!("Unregistering client from game");
        // Cancel if user is not game host
        if self.is_host
            && self
                .connected_clients
                .as_mut()
                .unwrap()
                .remove(client_id)
                .is_some()
        {
            // Send the disconnected client event to all connected clients
            let _ = self.send_global(
                DefaultModel::new(GameEvent::new(DisconnectedClientGameEvent {
                    game_id: self.game_id.clone(),
                    client_id: *client_id,
                })),
                &self.redis_pool,
            );
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
        trace!("Sending a global message to all clients in a game");

        // Send message to clients
        for clients in self
            .connected_clients
            .as_ref()
            .ok_or(ClientError::InternalServerError("not host"))?
            .iter()
        {
            // Todo: Better error handling when it fails to send message to client
            let _ = clients.1.send_message(message.clone(), redis_pool);
        }

        // Send message to host
        self.partial_host.send_message(message, redis_pool)?;

        Ok(())
    }

    /// Force shutdown the game
    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        trace!("Trying to shutdown a game");
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
                DefaultModel::new(GameEvent::new(ShutdownGameEvent {})),
                &self.redis_pool,
            );

            match res {
                Ok(_) => (),
                Err(e) => {
                    error!("Failed to send shutdown event to client{}", e);
                }
            }
        }

        self.connected_clients.as_mut().unwrap().clear();
        self.shutdown = true;

        // Deleting game from redis
        let mut conn = self.redis_pool.get()?;
        let _: () = conn.del(format!("GAME:{}", self.game_id))?;

        // Send final goodbye to the host
        self.partial_host.send_message(
            DefaultModel::new(Response::new(
                Some(ShutdownResponse { success: true }),
                models::ResponseOpCode::Shutdown,
            )),
            &self.redis_pool,
        )?;
        Ok(())
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        info!("Dropping a game object");
        if self.is_host {
            info!("Shutting down a game");
            let _ = self.shutdown();
            let mut conn = self.redis_pool.get().unwrap();
            let _: () = conn.del(format!("GAME:{}", self.game_id)).unwrap();
        } else if self.partial_host.is_local {
            info!("Leaving game, the host is local");
            if let Some(host_client) = &mut futures::executor::block_on(self.sockets.write())
                .get_mut(&self.partial_host.id)
            {
                if let Some(game) = &mut host_client.game {
                    game.unregister(&self.partial_client.id);
                }
            }

            let _ = self.partial_client.send_message(
                DefaultModel::new(Response::new(
                    Some(LeaveResponse { success: true }),
                    ResponseOpCode::Leave,
                )),
                &self.redis_pool,
            );
        } else {
            info!("Leaving a game on another shard");
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
