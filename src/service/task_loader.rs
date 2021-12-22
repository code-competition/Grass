use serde::{Deserialize, Serialize};

use super::websocket::client::game::task::GameTask;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileTaskList {
    tasks: Vec<GameTask>,
}

pub fn load_tasks(toml: &str) -> Vec<GameTask> {
    let tasks: FileTaskList = toml::from_str(toml).unwrap();

    println!("{:#?}", (tasks));

    Vec::new()
}
