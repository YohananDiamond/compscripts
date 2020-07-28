// TODO: make a date struct or something and add {defer dates, creation dates}

mod lib;
use clap::Clap;
use lib::{
    array_serialization::{ArrayArray, JsonArraySerializer},
    functions::{find_free_value, touch_read},
    getenv,
    traits::DataManager,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

fn main() {
    std::process::exit(TaskManager::start());
}

#[derive(Deserialize, Serialize, Eq, PartialEq)]
struct Task {
    id: u32,
    name: String,
    context: Option<String>,
    actionable: bool,
    children: Vec<Task>,
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

impl DataManager for TaskManager {
    type Data = Task;

    fn start() -> i32 {
        #[derive(Clap, Debug)]
        struct Opts {
            #[clap(
                short,
                long,
                about = "the path to the tasks file (default: $TKMN_FILE -> ~/.local/share/tkmn)"
            )]
            pub path: Option<String>,
            #[clap(subcommand, about = r#"What to do; defaults to "next""#)]
            pub subcmd: Option<SubCmd>,
        }

        #[derive(Clap, Debug)]
        enum SubCmd {
            #[clap(about = "Add a task")]
            Add(TaskMod),
            #[clap(about = "Select a task and do something with it")]
            Sel(SelCmd),
            #[clap(about = "List all active tasks in a tree format")]
            List,
            #[clap(about = "List next tasks")]
            Next,
            // TODO: search
        }

        #[derive(Clap, Debug)]
        struct SelCmd {
            #[clap(about = "The selection range")]
            pub selection: String, // TODO: make a clearer format (X..Y; X,Y; X)
            #[clap(subcommand)]
            pub action: ActionCmd,
        }

        #[derive(Clap, Debug)]
        enum ActionCmd {
            #[clap(about = "List all matches")]
            List,
            #[clap(about = "Add a subtask; only works if exactly one task is selected")]
            Sub(TaskMod),
            #[clap(about = "Modify a task")]
            Mod(TaskMod),
        }

        #[derive(Clap, Debug)]
        struct TaskMod {
            #[clap(about = "The name of the task")]
            name: Option<String>,
            #[clap(short, long, about = "The context of the task")]
            context: Option<String>,
            #[clap(short, long, about = "If the task is a note")]
            note: bool,
        }

        let options = Opts::parse();
        eprintln!("{:?}", options); // TODO: remove this

        let path_string = match options.path {
            Some(cfg) => cfg,
            None => getenv("TKMN_FILE")
                .unwrap_or(format!("{}/.local/share/tkmn", getenv("HOME").unwrap())),
        };
        let path = Path::new(&path_string);
        let contents = match touch_read(&path) {
            Ok(c) => {
                if c.chars()
                    .filter(|x| match x {
                        '\n' | ' ' => false,
                        _ => true,
                    })
                    .collect::<String>()
                    .len()
                    == 0
                {
                    String::from("[]")
                } else {
                    c
                }
            }
            Err(e) => {
                eprintln!("Failed to load task file: {}", e);
                return 1;
            }
        };

        let data: Vec<Task> = match ArrayArray::import(&contents) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to parse task file: {}", e);
                return 1;
            }
        };

        let mut manager = match TaskManager::new(data) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to set up bookmarks: {}", e);
                return 1;
            }
        };

        // TODO: ID duplication checking, used_ids and etc.

        let code: i32 = match options.subcmd {
            Some(SubCmd::Add(s)) => {
                if let Some(name) = s.name {
                    match manager.add_task(Task {
                        id: 0,
                        name: name,
                        context: s.context,
                        actionable: !s.note,
                        children: Vec::new(),
                    }) {
                        Ok(_) => {
                            eprintln!("Task added.");
                            0
                        }
                        Err(()) => {
                            eprintln!("Failed to add task.");
                            1
                        }
                    }
                } else {
                    eprintln!("The task name needs to be specified.");
                    return 1;
                }
            }
            // Some(SubCmd::Sel(s)) => manager.subcmd_addfromfile(&s),
            // Some(SubCmd::List) => manager.subcmd_report(Reports::All),
            // Some(SubCmd::Next) | None => manager.subcmd_report(Reports::Next),
            _ => 127,
        };

        if manager.modified {
            if let Err(e) = ArrayArray::new(&manager.data).save_to_file(&path) {
                eprintln!("Failed to save to file: {}", e);
                return 1;
            }
        }

        code
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
            // TODO: replace this with something more stables (sub-tasks)
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
    fn add_task(&mut self, task: Task) -> Result<u32, ()> {
        let id = find_free_value(&self.used_ids);
        self.data_mut().push(Task { id: id, ..task });
        self.modified = true;
        self.used_ids.insert(id);

        Ok(id)
    }
}

fn parse_range_str(string: &str) -> Result<Vec<u32>, String> {
    let mut result: Vec<u32> = Vec::new();
    let range_regex = Regex::new(r"^(\d+)\.\.(\d+)$").unwrap();
    let number_regex = Regex::new(r"^\d+$").unwrap();

    for number in string
        .chars()
        .filter(|x| *x != ' ')
        .collect::<String>()
        .split(",")
    {
        if number_regex.is_match(number) {
            result.push(number.parse::<u32>().unwrap())
        } else if range_regex.is_match(number) {
            let captures = range_regex.captures(number).unwrap();
            let num1: u32 = captures[1].parse().unwrap();
            let num2: u32 = captures[2].parse().unwrap();

            if num2 < num1 {
                return Err(format!(
                    "Second number {} is smaller than first number {} in range {}",
                    num2, num1, number
                ));
            }

            let mut i: u32 = num1;
            loop {
                result.push(i);
                i += 1;
                if i > num2 {
                    break;
                }
            }
        } else {
            return Err(format!("Could not parse {:?}", number));
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range() {
        let range_str = "1..10,4,5";
        assert_eq!(
            parse_range_str(range_str),
            Ok(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 4, 5])
        );
    }
}
