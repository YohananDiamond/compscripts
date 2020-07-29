// TODO: add a date struct - needed for defer dates and creation dates.
// TODO: pseudo-IDs
// TODO: make task_interact a little bit more clearer

mod lib;
use clap::Clap;
use lib::{
    array_serialization::{ArrayArray, JsonArraySerializer},
    misc::find_free_value,
    io::touch_read,
    aliases::getenv,
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

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
struct Task {
    id: u32,
    name: String,
    context: Option<String>,
    state: Option<bool>, // Some(b) means task (completed if b == true), None means note
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

impl Task {
    pub fn print(&self, level: usize) {
        println!(
            "{}({}) {} {} {}",
            std::iter::repeat(" ").take(level * 2).collect::<String>(),
            self.id,
            match self.state {
                None => "NOTE",
                Some(true) => "DONE",
                Some(false) => "TODO",
            },
            self.name,
            match self.context {
                Some(ref ctx) => format!("({})", ctx),
                None => format!(""),
            }
        );

        for child in &self.children {
            child.print(level + 1);
        }
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
            pub range: String,
            #[clap(subcommand, about = "What to do with the selection; defaults to list.")]
            pub action: Option<ActionCmd>,
        }

        #[derive(Clap, Debug)]
        enum ActionCmd {
            #[clap(about = "List all matches")]
            List,
            #[clap(about = "Add a subtask; only works if exactly one task is selected")]
            Sub(TaskMod),
            #[clap(about = "Modify a task")]
            Mod(TaskMod),
            Done,
            // Del, // TODO
        }

        #[derive(Clap, Debug)]
        struct TaskMod {
            #[clap(about = "The name of the task")]
            name: Option<String>,
            #[clap(short, long, about = "The context of the task")]
            context: Option<String>,
            #[clap(short, long, about = "If the task is a note")]
            note: Option<bool>,
        }

        let options = Opts::parse();

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
                eprintln!("Failed to set up manager: {}", e);
                return 1;
            }
        };

        let code: i32 = match options.subcmd {
            Some(SubCmd::Add(s)) => {
                if let Some(name) = s.name {
                    match manager.add_task(Task {
                        id: 0,
                        name: name,
                        context: s.context,
                        state: if s.note.unwrap_or(false) {
                            None
                        } else {
                            Some(false)
                        },
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
            Some(SubCmd::Sel(s)) => match parse_range_str(&s.range) {
                Ok(range) => {
                    if range.len() == 0 {
                        eprintln!("No selection was specified.");
                        1
                    } else {
                        match s.action.unwrap_or(ActionCmd::List) {
                            ActionCmd::List => {
                                let invalid_ids = manager.find_invalid_ids(&range[..]);
                                if invalid_ids.len() > 0 {
                                    eprintln!("Could not find task with IDs {:?}", invalid_ids);
                                    1
                                } else {
                                    manager
                                        .show_report("Selection Listing", &range[..])
                                        .unwrap();
                                    0
                                }
                            }
                            // ActionCmd::Sub(_) => 127, // TODO
                            // ActionCmd::Mod(_) => 127, // TODO
                            ActionCmd::Done => {
                                let invalid_ids = manager.find_invalid_ids(&range[..]);
                                if invalid_ids.len() > 0 {
                                    eprintln!("Could not find task with IDs {:?}", invalid_ids);
                                    1
                                } else {
                                    // Cool workaround for loop breaks.
                                    // https://github.com/rust-lang/rfcs/issues/961#issuecomment-264699920https://github.com/rust-lang/rfcs/issues/961#issuecomment-264699920
                                    'range: loop {
                                        for id in range {
                                            if manager
                                                .task_interact::<_, bool>(id, |t| t.state.is_none())
                                                .unwrap()
                                            {
                                                eprintln!("Item @[ID:{}] is a note and cannot be completed.", id);
                                                break 'range 1;
                                            } else {
                                                manager
                                                    .task_interact_mut(id, |t| {
                                                        t.state = Some(true);
                                                    })
                                                    .unwrap();
                                            }
                                        }
                                        break 'range 0;
                                    }
                                }
                            }
                            _ => unimplemented!(),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to parse range: {}", e);
                    1
                }
            },
            Some(SubCmd::List) => {
                let range = manager.get_surface_ids();
                manager
                    .show_report("Full surface listing", &range[..])
                    .unwrap(); // TODO: clarify somewhere what "surface" means
                0
            }
            Some(SubCmd::Next) | None => {
                // TODO: remake this to be like momentum.earth
                let range: Vec<u32> = manager
                    .get_surface_ids()
                    .iter()
                    .filter_map(|&id| {
                        manager
                            .task_interact::<_, Option<u32>>(id, |t| match t.state {
                                Some(true) => None,
                                _ => Some(id),
                            })
                            .unwrap()
                    })
                    .collect();

                manager.show_report("Next up", &range[..]).unwrap();
                0
            }
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
        taskvec_find_ids(&mut used_ids, &data)?;

        Ok(TaskManager {
            data: data,
            modified: false,
            used_ids: used_ids,
        })
    }

    pub fn find_task(&self, id: u32) -> Option<&Task> {
        // FIXME: this doesn't consider subtasks
        self.data().iter().find(|t| t.id == id)
    }

    pub fn find_task_mut(&mut self, id: u32) -> Option<&mut Task> {
        // FIXME: this doesn't consider subtasks
        self.data_mut().iter_mut().find(|t| t.id == id)
    }

    /// Returns Ok(()) in most situations.
    /// Returns Err(()) if `id` was not found.
    pub fn task_interact<F, T>(&self, id: u32, interaction: F) -> Result<T, ()>
    where
        F: Fn(&Task) -> T,
    {
        if let Some(task) = self.find_task(id) {
            Ok(interaction(task))
        } else {
            Err(())
        }
    }

    /// Returns Ok(()) in most situations.
    /// Returns Err(()) if `id` was not found.
    pub fn task_interact_mut<F, T>(&mut self, id: u32, interaction: F) -> Result<T, ()>
    where
        F: Fn(&mut Task) -> T,
    {
        if let Some(task) = self.find_task_mut(id) {
            let interaction_result = interaction(task);
            self.modified = true;
            Ok(interaction_result)
        } else {
            Err(())
        }
    }

    /// Returns `Err(id)` if id `id` was not found.
    pub fn show_report(&self, report_name: &str, ids: &[u32]) -> Result<(), u32> {
        println!("Report: {}", report_name);
        for id in ids {
            if self
                .task_interact(*id, |t| {
                    t.print(0);
                })
                .is_err()
            {
                return Err(*id);
            }
        }

        Ok(())
    }

    pub fn find_invalid_ids(&self, ids: &[u32]) -> Vec<u32> {
        ids.iter()
            .filter_map(|id| {
                if self.used_ids.contains(id) {
                    None
                } else {
                    Some(*id)
                }
            })
            .collect()
    }

    pub fn get_surface_ids(&self) -> Vec<u32> {
        self.data().iter().map(|t| t.id).collect()
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

fn taskvec_find_ids(set: &mut HashSet<u32>, data: &Vec<Task>) -> Result<(), String> {
    for task in data {
        if set.contains(&task.id) {
            return Err(format!(
                "found repeated ID: {} -- with task {:?}",
                task.id, task
            ));
        } else {
            set.insert(task.id);
        }

        // Recursively search for IDs in subtasks
        taskvec_find_ids(set, &task.children)?;
    }

    Ok(())
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
