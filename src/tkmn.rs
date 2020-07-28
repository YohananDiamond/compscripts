mod lib;
use lib::functions::find_free_value;
use lib::traits::{DataManager, JsonLines};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;

fn main() {
    std::process::exit(TaskManager::start());
}

#[derive(Deserialize, Serialize, Eq, PartialEq)]
struct Task {
    id: u32,
    name: String,
    project: String,
    due: Option<String>, // TODO: make a date struct or something
    created: String,
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct TaskManager {
    data: Vec<Task>,
    modified: bool,
    used_ids: HashSet<u32>,
}

impl<'a> JsonLines<'a> for TaskManager {}

impl DataManager for TaskManager {
    type Data = Task;

    fn start() -> i32 {
        0
    }

    fn data(&self) -> &Vec<Self::Data> {
        &self.data
    }

    fn data_mut(&mut self) -> &mut Vec<Self::Data> {
        &mut self.data
    }
}

impl TaskManager {
    pub fn new(data: Vec<Task>) -> Result<Self, String> {
        let mut used_ids: HashSet<u32> = HashSet::new();

        for task in data.iter() {
            if used_ids.contains(&task.id) {
                return Err(format!("repeated ID: {}", task.id));
            } else {
                used_ids.insert(task.id);
            }
        }

        Ok(TaskManager {
            data: data,
            modified: false,
            used_ids: used_ids,
        })
    }

    /// Note: the `id` field in `task` is ignored.
    fn add_task(&mut self, task: Task) -> Result<(), ()> {
        let id = find_free_value(&self.used_ids);
        self.data_mut().push(Task { id: id, ..task });
        self.used_ids.insert(id);

        Ok(())
    }
}
