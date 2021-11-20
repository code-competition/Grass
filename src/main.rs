use service::Service;

#[macro_use]
extern crate log;

mod service;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Generate random server (container instance) id
    let server_id = uuid::Uuid::new_v4().to_string();
    trace!("Generated server id for initialization: {}", &server_id);

    // Initialize service
    let mut service = Service::new(&server_id, "0.0.0.0:5000", "redis://127.0.0.1").await;

    // Run until finished
    service.run().await
}
