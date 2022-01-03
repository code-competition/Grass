#![deny(absolute_paths_not_starting_with_crate)]
#![deny(keyword_idents)]
#![deny(missing_copy_implementations)]
#![deny(missing_debug_implementations)]
#![warn(noop_method_call)]
#![deny(unused_import_braces)]
#![deny(unused_lifetimes)]

use std::path::Path;

use redis::Commands;
use serde::Deserialize;
use service::Service;

use crate::service::MiddlewareManager;

#[macro_use]
extern crate log;

mod middleware;
mod service;

#[derive(Deserialize, Debug)]
struct Config {
    address: String,
    port: u16,
    redis_addr: String,
}

#[derive(Deserialize, Debug)]
struct DebugConfig {
    should_reset_redis: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "grass");
    env_logger::init();

    // Env config
    let cfg = match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(_) => Config {
            address: "0.0.0.0".into(),
            port: 50000,
            redis_addr: "redis://127.0.0.1:35374".into(),
        },
    };

    // Env debug config
    if let Ok(config) = envy::from_env::<DebugConfig>() {
        if config.should_reset_redis {
            let client =
                redis::Client::open(cfg.redis_addr.to_string()).expect("redis connection failed");
            let mut con = client
                .get_connection()
                .expect("could not get redis connection");
            let _: () = con.set("GAME:monkey", "").unwrap();
        }
    };

    // Generate random shard (container instance) id
    let shard_id = uuid::Uuid::new_v4().to_string();
    trace!("Generated shard id for initialization: {}", &shard_id);

    let rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    rt.clone().block_on(async move {
        // Initialize service
        let host_addr = format!("{}:{}", cfg.address, cfg.port);
        let mut service = Service::new(
            &shard_id,
            &host_addr,
            Path::new("./tasks.toml"),
            &cfg.redis_addr,
        )
        .await;

        // enter the tokio runtime
        let _guard = rt.enter();

        // Run until finished
        let middleware = MiddlewareManager::new(middleware::shard_payload_interceptor);
        service.run(middleware).await
    })
}
