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
    models::{
        event::{
            disconnected_client::DisconnectedClientGameEvent, start::StartGameEvent,
            task_finished::TaskFinishedGameEvent,
        },
        response::compile::{progress::PublicTestProgress, CompilationResponse},
    },
    partial_client::PartialClient,
    sandbox::{sandbox_service_client::SandboxServiceClient, SandboxRequest, SandboxResponse},
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
    pub async fn start(
        &mut self,
        available_tasks: Arc<Vec<GameTask>>,
        task_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        superluminal_perf::begin_event("start game");
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
            client.task_progress = Some(HashMap::new());
            for task in self.tasks.iter().enumerate() {
                client.task_progress.as_mut().unwrap().insert(task.0, false);
            }
        }

        // Add tasks to host
        self.partial_host.task_progress = Some(HashMap::new());
        for task in self.tasks.iter().enumerate() {
            self.partial_host
                .task_progress
                .as_mut()
                .unwrap()
                .insert(task.0, false);
        }

        // Send game start notice to all clients
        let _ = self
            .send_global(
                DefaultModel::new(GameEvent::new(StartGameEvent { task_count })),
                None,
                &self.redis_pool,
            )
            .await;

        superluminal_perf::end_event();
        Ok(())
    }

    pub async fn prepare_code_test(
        &mut self,
        task_index: usize,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.is_host()?;
        let task = self
            .get_task_indexed(task_index)
            .map_err(|_| ClientError::OutOfRangeTask)?
            .to_owned();

        // Run all public tests, if they all succeed, run the private ones too
        Ok(task
            .public_test_cases
            .iter()
            .map(|test| test.stdin.to_owned())
            .collect::<Vec<String>>())
    }

    pub async fn run_code_test(
        client_id: &Uuid,
        code: String,
        gathered_stdin: Vec<String>,
    ) -> Result<SandboxResponse, Box<dyn std::error::Error>> {
        let result = Self::compile_code(client_id, code.clone(), gathered_stdin).await?;

        Ok(result)
    }

    /// Compile client code and return result
    pub async fn validate_code_test(
        &mut self,
        client_id: &Uuid,
        task_index: usize,
        code: String,
        result: SandboxResponse,
    ) -> Result<CompilationResponse, Box<dyn std::error::Error>> {
        superluminal_perf::begin_event("test code");
        // self.is_host()?;
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

        if !result.success {
            return Ok(CompilationResponse {
                task_index,
                public_test_progress: vec![],
                is_done: false,
                is_done_public_tests: false,
                is_done_private_tests: false,
                stderr: result.stderr.join(""),
            });
        }

        // ! Verify output against public tests
        let mut public_finished_tests = Vec::new();
        let mut public_failed_tests = Vec::new();
        for (i, mut s) in result.stdout.into_iter().enumerate() {
            if s.ends_with('\n') {
                s.pop();
            }

            if s == task
                .public_test_cases
                .get(i)
                .ok_or(ClientError::InternalServerError(
                    "task response length mismatch",
                ))?
                .expected
            {
                public_finished_tests.push((
                    task.public_test_cases
                        .get(i)
                        .ok_or(ClientError::InternalServerError(
                            "task response length mismatch",
                        ))?,
                    s,
                ));
            } else {
                public_failed_tests.push((
                    task.public_test_cases
                        .get(i)
                        .ok_or(ClientError::InternalServerError(
                            "task response length mismatch",
                        ))?,
                    s,
                ));
            }
        }

        // Check if all public tests succeeded
        if public_finished_tests.len() != task.public_test_cases.len() {
            let mut finished_public_tests = Vec::new();
            public_finished_tests.iter().for_each(|d| {
                finished_public_tests.push(PublicTestProgress::new_failed(
                    d.0.id,
                    d.1.clone(),
                    d.0.expected.to_owned(),
                ))
            });
            for (index, _) in public_finished_tests.iter().enumerate() {
                finished_public_tests
                    .get_mut(index)
                    .as_mut()
                    .unwrap()
                    .make_not_failed();
            }

            // add all the failed tests to the response
            public_failed_tests.iter().for_each(|d| {
                finished_public_tests.push(PublicTestProgress::new_failed(
                    d.0.id,
                    d.1.clone(),
                    d.0.expected.to_owned(),
                ))
            });

            // sort the tests by id
            finished_public_tests.sort_by(|x, y| x.test_index.cmp(&y.test_index));

            return Ok(CompilationResponse {
                task_index,
                public_test_progress: finished_public_tests,
                is_done: false,
                is_done_public_tests: false,
                is_done_private_tests: false,
                stderr: result.stderr.first().unwrap_or(&String::new()).to_string(),
            });
        }

        let mut finished_public_tests = Vec::new();
        public_finished_tests
            .into_iter()
            .enumerate()
            .for_each(|(x, d)| {
                finished_public_tests.push(PublicTestProgress::new(x, d.1, d.0.expected.to_owned()))
            });

        // If the public tests succeeded, test against the private test cases
        // Run all public tests, if they all succeed, run the private ones too
        let gathered_stdin = task
            .private_test_cases
            .iter()
            .map(|test| test.stdin.to_owned())
            .collect::<Vec<String>>();
        let result = Self::compile_code(client_id, code, gathered_stdin).await?;

        if !result.success {
            return Ok(CompilationResponse {
                task_index,
                public_test_progress: finished_public_tests,
                is_done: false,
                is_done_public_tests: true,
                is_done_private_tests: false,
                stderr: result.stderr.join(""),
            });
        }

        // Verify output against private tests
        let mut private_finished_tests = Vec::new();
        for (i, mut s) in result.stdout.into_iter().enumerate() {
            if s.ends_with('\n') {
                s.pop();
            }

            if s == task
                .private_test_cases
                .get(i)
                .ok_or(ClientError::InternalServerError(
                    "task response length mismatch",
                ))?
                .expected
            {
                private_finished_tests.push(task.private_test_cases.get(i).ok_or(
                    ClientError::InternalServerError("task response length mismatch"),
                )?);
            }
        }

        // Check if all public tests succeeded
        if private_finished_tests.len() != task.private_test_cases.len() {
            return Ok(CompilationResponse {
                task_index,
                public_test_progress: finished_public_tests,
                is_done: false,
                is_done_public_tests: true,
                is_done_private_tests: false,
                stderr: result.stderr.first().unwrap_or(&String::new()).to_string(),
            });
        }

        // Set the task as finished
        connected_client
            .task_progress
            .as_mut()
            .unwrap()
            .insert(task_index, true);

        // Send global event, client has succeeded with task
        let _ = self
            .send_global(
                DefaultModel::new(GameEvent::new(TaskFinishedGameEvent { task, task_index, client_id: *client_id })),
                Some(&[client_id]),
                &self.redis_pool,
            )
            .await;

        superluminal_perf::end_event();
        Ok(CompilationResponse {
            task_index,
            public_test_progress: finished_public_tests,
            is_done: true,
            is_done_public_tests: true,
            is_done_private_tests: true,
            stderr: String::new(),
        })
    }

    /// Compile client code and return result
    async fn compile_code(
        client_id: &Uuid,
        code: String,
        stdin: Vec<String>,
    ) -> Result<SandboxResponse, ClientError<'static>> {
        let mut client = SandboxServiceClient::connect("http://127.0.0.1:50051")
            .await
            .map_err(|_| ClientError::InternalServerError("internal compilation error"))?;
        let request = tonic::Request::new(SandboxRequest {
            user_id: client_id.to_string(),
            code,
            stdin,
            language: sandbox::Language::Rust as i32,
        });

        Ok((client.compile(request).await)
            .map_err(|_| ClientError::InternalServerError("internal compilation error"))?
            .into_inner())
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
        superluminal_perf::begin_event("register client");
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
                        nickname: clients.1.nickname.clone(),
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
                    nickname: self.partial_host.nickname.clone(),
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
                    nickname: partial_client.nickname.clone(),
                })),
                None,
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

        superluminal_perf::end_event();
        Ok(())
    }

    /// Unregister a client from the game
    pub async fn unregister(&mut self, client_id: &Uuid) {
        superluminal_perf::begin_event("unregister client");
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
                    None,
                    &self.redis_pool,
                )
                .await;
        }
        superluminal_perf::end_event();
    }

    /// Send a message to all connected clients in the game
    pub async fn send_global<'a, T>(
        &self,
        message: DefaultModel<T>,
        skip_client_ids: Option<&[&Uuid]>,
        redis_pool: &Pool<RedisConnectionManager>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize + Deserialize<'a> + Clone,
    {
        superluminal_perf::begin_event("send global within game");
        self.is_host()?;
        trace!("Sending a global message to all clients in a game");

        // Send message to clients
        for client in self
            .connected_clients
            .as_ref()
            .ok_or(ClientError::InternalServerError("not host"))?
            .iter()
        {
            if let Some(skip_client_ids) = skip_client_ids {
                if skip_client_ids.contains(&client.0) {
                    continue;
                }
            }

            if let Err(e) = client.1.send_message(message.clone(), redis_pool).await {
                error!(
                    "Failed to send global message to client with id {}, error {}",
                    client.0, e
                );
            }
        }

        // Send message to host
        let mut should_skip_host = false;
        if let Some(skip_client_ids) = skip_client_ids {
            if skip_client_ids.contains(&&self.partial_host.id) {
                should_skip_host = true;
            }
        }

        if !should_skip_host {
            self.partial_host.send_message(message, redis_pool).await?;
        }

        superluminal_perf::end_event();
        Ok(())
    }

    /// Force shutdown the game
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        superluminal_perf::begin_event("shutdown game");
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
        superluminal_perf::end_event();
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
        superluminal_perf::begin_event("dropping game");
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
        }
        superluminal_perf::end_event();
    }
}
