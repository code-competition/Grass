use serde::Deserialize;
use service::Service;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "grass");
    env_logger::init();

    // Env config
    let cfg = match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(_) => {
            Config {
                address: "0.0.0.0".into(),
                port: 5000,
                redis_addr: "redis://127.0.0.1".into()
            }
        }
    };

    // Generate random shard (container instance) id
    let shard_id = uuid::Uuid::new_v4().to_string();
    trace!("Generated shard id for initialization: {}", &shard_id);

    // Initialize service
    let host_addr = format!("{}:{}", cfg.address, cfg.port);
    let mut service = Service::new(&shard_id, &host_addr, &cfg.redis_addr).await;

    // Run until finished
    service
        .run(middleware::shard_payload_interceptor)
        .await
}
