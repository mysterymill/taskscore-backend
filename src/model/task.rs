
use std::{fmt::Display, sync::{Arc, Mutex}};

use bolt_client::bolt_proto::value::Node;
use chrono::{DateTime, Utc};
use rocket_okapi::okapi::schemars::JsonSchema;

use super::{entity::{Entity, FromInput, Relation}, User};

#[derive(serde::Serialize, Clone, JsonSchema, Debug)]
pub struct Task {
    pub id: Option<u32>,
    pub name: String,
    pub points: u16,
    pub enabled: bool,
}

impl Entity for Task {
    type I = u32;

    fn get_id(&self) -> &Option<u32>{
        &self.id
    }

    fn get_node_type_name() -> &'static str {
        "Task"
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_entity(f)
    }
}

impl From<Node> for Task {
    fn from(value: Node) -> Self {
        !unimplemented!();
    }
}

impl TryFrom<FromInput> for Task {
    type Error = String;
    fn try_from(input: FromInput) -> Result<Self, Self::Error> {
        let mut node_map = input.0;
        let task_node_opt = node_map.remove(Task::get_node_type_name());

        if task_node_opt.is_none() {
            return Err(String::from("Unable to create task from db node; no task nodes available"))
        }
        
        let mut task_node_vec = task_node_opt.unwrap();

        if task_node_vec.len() != 1 {
            return Err(format!("Unable to create task from db node; unusual number of task nodes: {}", task_node_vec.len()));
        }

        let task_node = task_node_vec.pop().unwrap();

        let task = Task::from(task_node);
        Ok(task)
    }
}

#[derive(serde::Serialize, Clone, JsonSchema, Debug)]
pub struct Score {
    pub task: Arc<Mutex<Task>>,
    pub points: u16,
    pub scored_at: DateTime::<Utc>,
}

impl Score {
    pub fn new(task: Arc<Mutex<Task>>) -> Score {
        let points = task.lock().unwrap().points.clone();
        Score { task: task.clone(), points, scored_at: chrono::Utc::now()}
    }
}

impl From<Relation<User, Task>> for Score {
    
    fn from(value: Relation<User, Task>) -> Self {
        let task = value.target_node;
        Score::new(task.clone())
    }
}