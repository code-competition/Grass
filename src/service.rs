pub struct Service<'a> {
    server_id: &'a str
}

impl<'a> Service<'a> {
    pub async fn new(server_id: &'a str) -> Service<'a> {
        Self {
            server_id
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}