use std::{collections::HashMap, sync::Arc};

use r2d2::Pool;
use rand::prelude::SliceRandom;
use redis::Commands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
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
    sandbox::{sandbox_service_client::SandboxServiceClient, SandboxRequest, SandboxResponse},
    task::{progress::TaskProgress, GameTask},
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
pub struct CompilationResult {
    task_index: usize,
    task_progress: Vec<(usize, bool)>,
    is_done: bool,
    stdout: String,
    stderr: String,
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
    pub async fn start(
        &mut self,
        available_tasks: Arc<Vec<GameTask>>,
        task_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.is_host()?;
        if self.is_started {
            return Err(Box::new(ClientError::GameAlreadyStarted));
        }

        self.public = false;
        self.is_started = true;

        // Choose a random programming question
        self.tasks = available_tasks
            .choose_multiple(&mut rand::thread_rng(), task_count)
            .map(|x| x.to_owned())
            .collect();

        // Verify that the tasks have been filled up
        if self.tasks.len() != task_count {
            return Err(Box::new(ClientError::InternalServerError(
                "Internal server error, requested task count could not be filled",
            )));
        }

        // Add tasks to all connected clients
        for client in self.connected_clients.as_mut().unwrap().values_mut() {
            for task in self.tasks.iter().enumerate() {
                client.task_progress.as_mut().unwrap().insert(
                    task.0,
                    TaskProgress {
                        finished_first: false,
                        finished_all: false,
                    },
                );
            }
        }

        // Add taks to host
        self.partial_host.task_progress = Some(HashMap::new());
        for task in self.tasks.iter().enumerate() {
            self.partial_host.task_progress.as_mut().unwrap().insert(
                task.0,
                TaskProgress {
                    finished_first: false,
                    finished_all: false,
                },
            );
        }

        // Get the first task to send to all clients
        let task = self.get_task_indexed(0).unwrap();

        // Send game start notice to all clients
        let _ = self
            .send_global(
                DefaultModel::new(GameEvent::new(StartGameEvent { task_count })),
                &self.redis_pool,
            )
            .await;

        // Send the first task to all clients
        let _ = self
            .send_global(
                DefaultModel::new(GameEvent::new(TaskGameEvent {
                    task: task.to_owned(),
                })),
                &self.redis_pool,
            )
            .await;

        Ok(())
    }

    /// Compile client code and return result
    pub async fn test_code(
        &mut self,
        client_id: &Uuid,
        code: String,
        task_index: usize,
    ) -> Result<CompilationResult, Box<dyn std::error::Error>> {
        self.is_host()?;
        let task = self
            .get_task_indexed(task_index)
            .map_err(|_| ClientError::OutOfRangeTask)?
            .to_owned();

        let connected_client = if !self.is_host {
            self.connected_clients
                .as_mut()
                .unwrap()
                .get_mut(client_id)
                .ok_or(ClientError::ClientDoesNotExist(
                    "Client does not exist in the game",
                ))?
        } else {
            &mut self.partial_host
        };

        let task_progress = connected_client
            .task_progress
            .as_mut()
            .unwrap()
            .get_mut(&task_index)
            .unwrap();

        if !task_progress.finished_first {
            // Run first test
            let test_case = task.test_cases.get(0).unwrap();
            let result = Self::compile_code(client_id, code, test_case.stdin.to_string())
                .await
                .map_err(|_| ClientError::InternalServerError("failed to compile"))?;

            // It failed, return stderr
            if !result.success {
                let stderr = result.stderr.join("");
                return Ok(CompilationResult {
                    task_index,
                    task_progress: vec![],
                    is_done: false,
                    stdout: String::new(),
                    stderr,
                });
            } else {
                let mut stdout = result.stdout.join("");
                // Clean output
                if stdout.ends_with('\n') {
                    stdout.pop();
                }
                if stdout.ends_with('\r') {
                    stdout.pop();
                }

                if stdout == test_case.expected {
                    task_progress.finished_first = true;
                    return Ok(CompilationResult {
                        task_index,
                        task_progress: vec![(0, true)],
                        is_done: false,
                        stdout,
                        stderr: String::new(),
                    });
                } else {
                    task_progress.finished_first = false;
                    return Ok(CompilationResult {
                        task_index,
                        task_progress: vec![(0, false)],
                        is_done: false,
                        stdout,
                        stderr: String::new(),
                    });
                }
            }
        } else {
            // Run all tests
            for tests in task.test_cases.into_iter() {
                // break if any of them fail
            }
        }

        Ok(CompilationResult {
            task_index,
            task_progress: todo!(),
            is_done: todo!(),
            stdout: todo!(),
            stderr: todo!(),
        })
    }

    /// Compile client code and return result
    async fn compile_code(
        client_id: &Uuid,
        code: String,
        stdin: String,
    ) -> Result<SandboxResponse, Box<dyn std::error::Error>> {
        let mut client = SandboxServiceClient::connect("http://127.0.0.1:50051").await?;
        let request = tonic::Request::new(SandboxRequest {
            user_id: client_id.to_string(),
            code,
            stdin,
            language: sandbox::Language::Rust as i32,
        });

        Ok(client.compile(request).await?.into_inner())
    }

    /// Fetches a task at index
    pub fn get_task_indexed(&self, index: usize) -> Result<&GameTask, Option<()>> {
        if self.is_started {
            Ok(self.tasks.get(index).ok_or(Some(()))?)
        } else {
            Err(None)
        }
    }

    /// Register a new client with the game
    pub async fn register(&mut self, partial_client: PartialClient) -> Result<(), ()> {
        trace!("Registering client from game");
        // Cancel if user is not game host
        if !self.is_host || !self.public {
            return Err(());
        }

        // Send existing clients to the newly connected client
        for clients in self.connected_clients.as_ref().unwrap().iter() {
            let _ = partial_client
                .send_message(
                    DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                        game_id: self.game_id.clone(),
                        client_id: *clients.0,
                    })),
                    &self.redis_pool,
                )
                .await;
        }

        // Send the host to the newly connected client
        let _ = partial_client
            .send_message(
                DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                    game_id: self.game_id.clone(),
                    client_id: self.partial_host.id,
                })),
                &self.redis_pool,
            )
            .await;

        // Send the new client to all connected clients
        let _ = self
            .send_global(
                DefaultModel::new(GameEvent::new(ConnectedClientGameEvent {
                    game_id: self.game_id.clone(),
                    client_id: partial_client.id,
                })),
                &self.redis_pool,
            )
            .await;

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
    pub async fn unregister(&mut self, client_id: &Uuid) {
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
            let _ = self
                .send_global(
                    DefaultModel::new(GameEvent::new(DisconnectedClientGameEvent {
                        game_id: self.game_id.clone(),
                        client_id: *client_id,
                    })),
                    &self.redis_pool,
                )
                .await;
        }
    }

    /// Send a message to all connected clients in the game
    pub async fn send_global<'a, T>(
        &self,
        message: DefaultModel<T>,
        redis_pool: &Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize + Deserialize<'a> + Clone,
    {
        self.is_host()?;
        trace!("Sending a global message to all clients in a game");

        // Send message to clients
        for client in self
            .connected_clients
            .as_ref()
            .ok_or(ClientError::InternalServerError("not host"))?
            .iter()
        {
            // Todo: Better error handling when it fails to send message to client
            if let Err(e) = client.1.send_message(message.clone(), redis_pool).await {
                error!(
                    "Failed to send global message to client with id {}, error {}",
                    client.0, e
                );
            }
        }

        // Send message to host
        self.partial_host.send_message(message, redis_pool).await?;

        Ok(())
    }

    /// Force shutdown the game
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.is_host()?;

        trace!(
            "Host \"{}\" triggered a shutdown event",
            &self.partial_host.id
        );
        for client in self.connected_clients.as_ref().unwrap().iter() {
            let partial = client.1;
            if let Err(e) = partial
                .send_message(
                    DefaultModel::new(GameEvent::new(ShutdownGameEvent {})),
                    &self.redis_pool,
                )
                .await
            {
                error!("Failed to send shutdown event to client{}", e);
            }
        }

        self.connected_clients.as_mut().unwrap().clear();
        self.shutdown = true;

        // Deleting game from redis
        let mut conn = self.redis_pool.get()?;
        let _: () = conn.del(format!("GAME:{}", self.game_id))?;

        // Send final goodbye to the host
        self.partial_host
            .send_message(
                DefaultModel::new(Response::new(
                    Some(ShutdownResponse { success: true }),
                    models::ResponseOpCode::Shutdown,
                )),
                &self.redis_pool,
            )
            .await?;
        Ok(())
    }

    fn is_host(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_host {
            Err(Box::new(ClientError::NotGameHost(
                "Client is not the game host",
            )))
        } else {
            Ok(())
        }
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        info!("Dropping a game object");
        if self.is_host {
            info!("Shutting down a game");
            let _ = futures::executor::block_on(self.shutdown());
            let mut conn = self.redis_pool.get().unwrap();
            let _: () = conn.del(format!("GAME:{}", self.game_id)).unwrap();
        } else if self.partial_host.is_local {
            info!("Leaving game, the host is local");
            if let Some(host_client) = &mut self.sockets.get_mut(&self.partial_host.id) {
                if let Some(game) = &mut host_client.game {
                    futures::executor::block_on(game.unregister(&self.partial_client.id));
                }
            }

            let _ = futures::executor::block_on(self.partial_client.send_message(
                DefaultModel::new(Response::new(
                    Some(LeaveResponse { success: true }),
                    ResponseOpCode::Leave,
                )),
                &self.redis_pool,
            ));
        } else {
            // info!("Leaving a game on another shard");
            // // If the client is not host, disconnect it from the game
            // let request = ShardRequest::new(
            //     ShardLeaveRequest {
            //         game_id: self.game_id.to_string(),
            //         client_id: self.partial_client.id,
            //         host_id: self.partial_host.id,
            //         shard_id: Uuid::from_str(&self.partial_client.shard_id).unwrap(),
            //     },
            //     ShardRequestOpCode::Leave,
            // );

            // // Send request to shard hosting the game
            // let _ = sharding::send_redis(
            //     &self.redis_pool,
            //     (
            //         None,
            //         Some(Uuid::from_str(&self.partial_host.shard_id).unwrap()),
            //     ),
            //     request,
            //     ShardOpCode::Request,
            // );
        }
    }
}
