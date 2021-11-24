use service::Service;

#[macro_use]
extern crate log;

mod service;
mod middleware;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "grass");

    env_logger::init();

    // Generate random shard (container instance) id
    let shard_id = uuid::Uuid::new_v4().to_string();
    trace!("Generated shard id for initialization: {}", &shard_id);

    // Initialize service
    let mut service = Service::new(&shard_id, "0.0.0.0:5000", "redis://127.0.0.1").await;

    // Run until finished
    service.run(middleware::shard_payload_interceptor::<String>).await
}
