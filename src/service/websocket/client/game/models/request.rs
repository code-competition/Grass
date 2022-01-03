use std::sync::Arc;

use r2d2::Pool;
use redis::Commands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::service::{
    redis_pool::RedisConnectionManager,
    websocket::client::{
        error::ClientError,
        game::{partial_client::PartialClient, redis_game::RedisGame, task::GameTask, Game},
        models::{DefaultModel, OpCode, OpCodeFetcher},
    },
    Sockets,
};

use self::{
    compile::CompileRequest, create::CreateRequest, exists::ExistsRequest,
    identify::IdentifyRequest, join::JoinRequest, start::StartRequest, task::TaskRequest,
};

use super::{
    response::{
        compile::CompilationResponse, create::CreateResponse, exists::ExistsResponse,
        identify::IdentifyResponse, join::JoinResponse, ping::PingResponse, task::TaskResponse,
    },
    Response, ResponseOpCode,
};

pub mod compile;
pub mod create;
pub mod exists;
pub mod identify;
pub mod join;
pub mod leave;
pub mod ping;
pub mod start;
pub mod task;

// Models for requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    d: Option<Value>,
    op: RequestOpCode,
}

impl Request {
    pub async fn handle_message<'a>(
        self,
        client_id: Uuid,
        sockets: &Sockets,
        redis_pool: Pool<RedisConnectionManager>,
        available_tasks: Arc<Vec<GameTask>>,
        shard_id: &str,
    ) -> Result<(), ClientError<'a>> {
        match self.op {
            RequestOpCode::Join => {
                if self.d.is_none() {
                    return Err(ClientError::NoDataWithOpCode(
                        "No data was sent with join request",
                    ));
                }

                // Parse the join request
                let join_game: JoinRequest = serde_json::from_value(self.d.unwrap())
                    .map_err(|_| ClientError::ParsingError)?;

                let client_write_channel;
                let mut conn;
                let game;
                let nickname;
                {
                    let client = sockets.get(&client_id).unwrap();
                    client_write_channel = client.send_channel.clone();

                    // Check if user is already in a game
                    if client.game.is_some() {
                        let _ = client
                            .send_error(ClientError::AlreadyInGame("Client is already in a game"))
                            .await;
                        return Err(ClientError::AlreadyInGame("Client is already in a game"));
                    }

                    // Check if the client has identified itself
                    if client.nickname.is_none() {
                        let _ = client.send_error(ClientError::ClientNotIdentified).await;
                        return Err(ClientError::ClientNotIdentified);
                    } else {
                        nickname = client.nickname.as_ref().unwrap().clone();
                    }

                    // get a redis connection from the pool
                    conn = match redis_pool.get() {
                        Ok(c) => c,
                        Err(_) => {
                            let _ = client
                                .send_error(ClientError::InternalServerError(
                                    "Internal Server Error",
                                ))
                                .await;
                            return Err(ClientError::InternalServerError("Internal Server Error"));
                        }
                    };

                    // Try to fetch the game from redis
                    let redis_game: redis::RedisResult<String> =
                        conn.get(format!("GAME:{}", join_game.game_id));
                    game = match redis_game {
                        Ok(game) => game,
                        Err(_) => {
                            error!("No game was found");
                            let _ = client.send_error(ClientError::NoGameWasFound).await;
                            return Err(ClientError::NoGameWasFound);
                        }
                    };

                    drop(client);
                }

                // Check if the game has been initialized
                // If it'sockets not, set self as host and join game
                if game == String::new() {
                    // Serialize game object data
                    let redis_game = RedisGame {
                        shard_id: (*shard_id).to_string(),
                        host_id: client_id,
                    };
                    let serialized_redis_game = serde_json::to_string(&redis_game).unwrap();

                    // Register as host
                    let _: () = conn
                        .set(format!("GAME:{}", join_game.game_id), serialized_redis_game)
                        .map_err(|_| ClientError::InternalServerError("Internal cache error"))?;

                    let mut client = sockets.get_mut(&client_id).unwrap();
                    client.game = Some(Game::new(
                        true,
                        join_game.game_id.clone(),
                        PartialClient::new(
                            client.id,
                            client.nickname.as_ref().unwrap().to_owned(),
                            redis_game.shard_id.clone(),
                            true,
                            Some(client.send_channel.clone()),
                        ),
                        PartialClient::new(
                            client.id,
                            client.nickname.as_ref().unwrap().to_owned(),
                            redis_game.shard_id,
                            true,
                            Some(client.send_channel.clone()),
                        ),
                        redis_pool,
                        sockets.clone(),
                    ));

                    client
                        .send_model(DefaultModel::new(Response::new(
                            Some(JoinResponse {
                                game_id: join_game.game_id,
                                is_host: true,
                                success: true,
                            }),
                            ResponseOpCode::Join,
                        )))
                        .await
                        .map_err(|_| ClientError::SendError)?;
                } else {
                    // Should join an already existing game through the shard communication protocol (redis)
                    // Or by doing it locally, if the game is hosted on the same server as the socket client
                    let redis_game: RedisGame = serde_json::from_str(&game)
                        .map_err(|_| ClientError::InternalServerError("Failed to parse game"))?;

                    // Check if game is on the same server
                    if redis_game.shard_id == shard_id {
                        // Todo: this is where the issue is, sockets got fetching two times for mutable
                        let mut response = None;
                        let game_host_send_channel;

                        let host_nickname;
                        {
                            let host = sockets.get_mut(&redis_game.host_id);
                            match host {
                                Some(mut game_host_client) => {
                                    host_nickname =
                                        game_host_client.nickname.as_ref().unwrap().clone();
                                    game_host_send_channel =
                                        Some(game_host_client.send_channel.clone());
                                    if let Some(game_host_client_game) = &mut game_host_client.game
                                    {
                                        // Register the local client in the host game
                                        if game_host_client_game
                                            .register(PartialClient::new(
                                                client_id,
                                                nickname.to_owned(),
                                                shard_id.to_string(),
                                                true,
                                                Some(client_write_channel),
                                            ))
                                            .await
                                            .is_ok()
                                        {
                                            response = Some(DefaultModel::new(Response::new(
                                                Some(JoinResponse {
                                                    game_id: join_game.game_id.clone(),
                                                    is_host: false,
                                                    success: true,
                                                }),
                                                ResponseOpCode::Join,
                                            )));
                                        }
                                    } else {
                                        let _: redis::RedisResult<()> =
                                            conn.del(format!("GAME:{}", join_game.game_id));
                                        return Err(ClientError::InternalServerError(
                                            "Host was not in the game, could not join it.",
                                        ));
                                    }
                                }
                                None => {
                                    // Unregister the game if the host is gone
                                    let _: redis::RedisResult<()> =
                                        conn.del(format!("GAME:{}", join_game.game_id));
                                    return Err(ClientError::ClientDoesNotExist(
                                        "Socket client does not exist",
                                    ));
                                }
                            }
                        }

                        // Continue here
                        let mut client = sockets.get_mut(&client_id).unwrap();
                        if response.is_some() {
                            // Register the game for the client
                            client.game = Some(Game::new(
                                false,
                                join_game.game_id.clone(),
                                PartialClient::new(
                                    client.id,
                                    nickname.to_owned(),
                                    shard_id.to_string(),
                                    true,
                                    Some(client.send_channel.clone()),
                                ),
                                PartialClient::new(
                                    redis_game.host_id,
                                    host_nickname,
                                    shard_id.to_string(),
                                    true,
                                    Some(game_host_send_channel.unwrap()),
                                ),
                                redis_pool,
                                sockets.clone(),
                            ));
                        } else {
                            response = Some(DefaultModel::new(Response::new(
                                Some(JoinResponse {
                                    game_id: join_game.game_id,
                                    is_host: false,
                                    success: false,
                                }),
                                ResponseOpCode::Join,
                            )));
                        }

                        client
                            .send_model(response.unwrap())
                            .await
                            .map_err(|_| ClientError::SendError)?;
                    } else {
                        let mut conn = redis_pool.get().unwrap();
                        let _: () = conn
                            .del(join_game.game_id.clone())
                            .map_err(|_| ClientError::InternalServerError("Game was corrupt"))?;
                        let client = sockets.get(&client_id).unwrap();
                        client
                            .send_model(DefaultModel::new(Response::new(
                                Some(JoinResponse {
                                    game_id: join_game.game_id,
                                    is_host: false,
                                    success: false,
                                }),
                                ResponseOpCode::Join,
                            )))
                            .await
                            .map_err(|_| ClientError::SendError)?;
                        return Err(ClientError::NoGameWasFound);
                    }
                }
            }
            RequestOpCode::Leave => {
                let mut client = sockets.get_mut(&client_id).unwrap();

                trace!("Received request to leave a game");
                if client.game.is_none() {
                    let _ = client
                        .send_error(ClientError::NotInGame("Client was not in a game"))
                        .await;
                    return Err(ClientError::NotInGame("Client was not in a game"));
                }

                // The client must be in a game at this point, it'sockets safe to unwrap the value
                let game = client.game.take().unwrap();

                // Dropping the game object will leave the game cleanly, or shut it down if the client was host
                drop(game);
            }
            RequestOpCode::Start => {
                let mut client = sockets.get_mut(&client_id).unwrap();
                if client.game.is_none() {
                    let _ = client
                        .send_error(ClientError::NotInGame("Client was not in a game"))
                        .await;
                    return Err(ClientError::NotInGame("Client was not in a game"));
                } else if !client.game.as_ref().unwrap().is_host {
                    // Dropping the game object will leave the game cleanly
                    let game = client.game.take().unwrap();
                    drop(game);

                    let _ = client
                        .send_error(ClientError::NotGameHost("Client was not the game host"))
                        .await;
                    return Err(ClientError::NotGameHost("Client was not the game host"));
                }

                // Parse the request
                let request: StartRequest = serde_json::from_value(self.d.unwrap())
                    .map_err(|_| ClientError::ParsingError)?;

                // Start the game
                client
                    .game
                    .as_mut()
                    .unwrap()
                    .start(available_tasks, request.task_count)
                    .await
                    .map_err(|_| ClientError::SendError)?;
            }
            RequestOpCode::Task => {
                let client = sockets.get(&client_id).unwrap();
                if client.game.is_none() {
                    let _ = client
                        .send_error(ClientError::NotInGame("Client was not in a game"))
                        .await;
                    return Err(ClientError::NotInGame("Client was not in a game"));
                }

                // Parse the request
                let request: TaskRequest = serde_json::from_value(self.d.unwrap())
                    .map_err(|_| ClientError::ParsingError)?;

                let host_id = client.game.as_ref().unwrap().partial_host.id;
                if let Some(host) = sockets.get(&host_id) {
                    if let Some(game) = &host.game {
                        match game.get_task_indexed(request.task_index) {
                            Ok(task) => {
                                client
                                    .send_model(DefaultModel::new(Response::new(
                                        Some(TaskResponse {
                                            task: task.to_owned(),
                                        }),
                                        ResponseOpCode::Task,
                                    )))
                                    .await
                                    .map_err(|_| ClientError::SendError)?;
                            }
                            Err(e) => match e {
                                Some(_) => {
                                    let error = ClientError::OutOfRangeTask;
                                    let _ = client.send_error(error.clone()).await;
                                    return Err(error);
                                }
                                None => {
                                    let error = ClientError::GameNotStarted;
                                    let _ = client.send_error(error.clone()).await;
                                    return Err(error);
                                }
                            },
                        }
                    } else {
                        let _ = client
                            .send_error(ClientError::InternalServerError(
                                "Host was not in the same game",
                            ))
                            .await;
                        return Err(ClientError::InternalServerError(
                            "Host was not in the same game",
                        ));
                    }
                } else {
                    let _ = client
                        .send_error(ClientError::InternalServerError("Host does not exist"))
                        .await;
                    return Err(ClientError::InternalServerError("Host does not exist"));
                }
            }
            RequestOpCode::Compile => {
                trace!("Received compilation request");
                // Check if client is in game and if so, return game host id
                let host_id = {
                    let client = sockets.get(&client_id).unwrap();
                    if client.game.is_none() {
                        let _ = client
                            .send_error(ClientError::NotInGame("Client was not in a game"))
                            .await;
                        return Err(ClientError::NotInGame("Client was not in a game"));
                    }

                    client.game.as_ref().unwrap().partial_host.id
                };

                // Parse the request
                let request: CompileRequest = serde_json::from_value(self.d.unwrap())
                    .map_err(|_| ClientError::ParsingError)?;

                let response: (Option<CompilationResponse>, Option<ClientError>) = {
                    if let Some(host) = &mut sockets.get_mut(&host_id) {
                        if let Some(game) = &mut host.game {
                            if !game.is_started {
                                (None, Some(ClientError::GameNotStarted))
                            } else {
                                match game
                                    .test_code(&client_id, request.code, request.task_index)
                                    .await
                                {
                                    Ok(r) => (Some(r), None),
                                    Err(_) => (
                                        None,
                                        Some(ClientError::CompilationError("failed to compile")),
                                    ),
                                }
                            }
                        } else {
                            (
                                None,
                                Some(ClientError::InternalServerError("Host was not in the game")),
                            )
                        }
                    } else {
                        (
                            None,
                            Some(ClientError::InternalServerError("Host does not exist")),
                        )
                    }
                };

                if let Some(error) = response.1 {
                    let client = sockets.get(&client_id).unwrap();
                    let _ = client.send_error(error.clone()).await;
                    return Err(error);
                } else if let Some(response) = response.0 {
                    trace!("Finished compilation successfully");
                    let client = sockets.get(&client_id).unwrap();
                    client
                        .send_model(DefaultModel::new(Response::new(
                            Some(response),
                            ResponseOpCode::Compile,
                        )))
                        .await
                        .map_err(|_| ClientError::SendError)?;
                }
            }
            RequestOpCode::Ping => {
                let client = sockets.get(&client_id).unwrap();
                client
                    .send_model(DefaultModel::new(Response::new(
                        Some(PingResponse {}),
                        ResponseOpCode::Ping,
                    )))
                    .await
                    .map_err(|_| ClientError::SendError)?;
            }
            RequestOpCode::Identify => {
                let mut client = sockets.get_mut(&client_id).unwrap();
                if client.nickname.is_none() {
                    let request: IdentifyRequest = serde_json::from_value(self.d.unwrap())
                        .map_err(|_| ClientError::ParsingError)?;

                    // Todo: check how appropiate a nickname is
                    client.nickname = Some(request.nickname);
                    client
                        .send_model(DefaultModel::new(Response::new(
                            Some(IdentifyResponse { success: true }),
                            ResponseOpCode::Identify,
                        )))
                        .await
                        .map_err(|_| ClientError::SendError)?;
                } else {
                    client
                        .send_model(DefaultModel::new(Response::new(
                            Some(IdentifyResponse { success: false }),
                            ResponseOpCode::Identify,
                        )))
                        .await
                        .map_err(|_| ClientError::SendError)?;
                }
            }
            RequestOpCode::Create => {
                let _: CreateRequest = serde_json::from_value(self.d.unwrap())
                    .map_err(|_| ClientError::ParsingError)?;

                let alphabet: &[char] = &['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];
                let game_id = nanoid::nanoid!(10, alphabet);

                let mut conn = redis_pool
                    .get()
                    .map_err(|_| ClientError::InternalServerError("Cache error"))?;
                let _: () = conn
                    .set(format!("GAME:{}", game_id.clone()), "")
                    .map_err(|_| ClientError::InternalServerError("Cache error"))?;

                let client = sockets.get(&client_id).unwrap();
                client
                    .send_model(DefaultModel::new(Response::new(
                        Some(CreateResponse { game_id }),
                        ResponseOpCode::Create,
                    )))
                    .await
                    .map_err(|_| ClientError::SendError)?;
            }
            RequestOpCode::Exists => {
                let request: ExistsRequest = serde_json::from_value(self.d.unwrap())
                    .map_err(|_| ClientError::ParsingError)?;

                let mut conn = redis_pool
                    .get()
                    .map_err(|_| ClientError::InternalServerError("Cache error"))?;

                // check if game exists in redis
                let res = match conn.get::<String, String>(format!("GAME:{}", request.game_id)) {
                    Ok(game) => match game.as_str() {
                        "" => ExistsResponse { exists: true },
                        _ => {
                            // Verify that game is still valid
                            let redis_game: RedisGame =
                                serde_json::from_str(&game).map_err(|_| {
                                    ClientError::InternalServerError("Failed to parse game")
                                })?;

                            // Remove the game if the host has left but the service failed to remove the game
                            if sockets.get(&redis_game.host_id).is_none() {
                                let _: redis::RedisResult<()> =
                                    conn.del(format!("GAME:{}", request.game_id));

                                ExistsResponse { exists: false }
                            } else {
                                ExistsResponse { exists: true }
                            }
                        }
                    },
                    Err(_) => ExistsResponse { exists: false },
                };

                let client = sockets.get(&client_id).unwrap();
                client
                    .send_model(DefaultModel::new(Response::new(
                        Some(res),
                        ResponseOpCode::Exists,
                    )))
                    .await
                    .map_err(|_| ClientError::SendError)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RequestOpCode {
    Join,
    Leave,
    Start,
    Task,
    Compile,
    Ping,
    Identify,
    Create,
    Exists,
}

impl OpCodeFetcher for Request {
    #[inline]
    fn op_code() -> OpCode {
        OpCode::Request
    }
}
