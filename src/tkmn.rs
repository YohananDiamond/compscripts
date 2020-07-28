// TODO: make a date struct or something and add {defer dates, creation dates}

mod lib;
use lib::functions::find_free_value;
use lib::traits::{DataManager, JsonLines};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use clap::Clap;

fn main() {
    std::process::exit(TaskManager::start());
}

#[derive(Deserialize, Serialize, Eq, PartialEq)]
struct Task {
    id: u32,
    name: String,
    context: String,
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

impl<'a> JsonLines<'a> for TaskManager {}

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
            #[clap(subcommand)]
            pub subcmd: SubCmd
        }

        #[derive(Clap, Debug)]
        enum SubCmd {
            #[clap(about = "adds a task")]
            Add(TaskMod),
            #[clap(about = "selects a task and does something with it")]
            Sel(SelCmd),
            #[clap(about = "lists all active tasks in a tree format")]
            All,
            #[clap(about = "lists next tasks")]
            Next,
            // TODO: search
        }

        #[derive(Clap, Debug)]
        struct SelCmd {
            #[clap(about = "the selection range")]
            pub selection: String, // TODO: make a clearer format (X..Y; X,Y; X)
            #[clap(subcommand)]
            pub action: ActionCmd,
        }

        #[derive(Clap, Debug)]
        enum ActionCmd {
            #[clap(about = "shows the matches")]
            List,
            #[clap(about = "adds a subtask; only works if exactly one task is selected")]
            Sub(TaskMod),
            #[clap(about = "modifies the task")]
            Mod(TaskMod),
        }

        #[derive(Clap, Debug)]
        struct TaskMod {
            #[clap(about = "the name of the task")]
            name: Option<String>,
            #[clap(short, long, about = "the context of the task")]
            context: Option<String>,
            #[clap(short, long, about = "if the task is a note")]
            note: bool,
        }

        let options = Opts::parse();
        eprintln!("{:?}", options);

        // let path_string = match options.path {
        //     Some(cfg) => cfg,
        //     None => std::env::var("BKMK_FILE").unwrap_or(format!(
        //         "{}/.local/share/bkmk",
        //         std::env::var("HOME").unwrap()
        //     )),
        // };
        // let path = Path::new(&path_string);
        // let contents = match touch_read(&path) {
        //     Ok(c) => c,
        //     Err(e) => {
        //         eprintln!("Failed to load bookmarks file: {}", e);
        //         return 1;
        //     }
        // };

        // let data: Vec<Bookmark> = match Self::from_json_lines(&contents) {
        //     Ok(o) => o,
        //     Err(e) => {
        //         eprintln!("Failed to parse bookmarks file: {}", e);
        //         return 1;
        //     }
        // };

        // let mut manager = match BookmarkManager::new(data) {
        //     Ok(o) => o,
        //     Err(e) => {
        //         eprintln!("Failed to set up bookmarks: {}", e);
        //         return 1;
        //     }
        // };

        // let code: i32 = match options.subcmd {
        //     SubCmd::Add(s) => manager.subcmd_add(&s.url),
        //     SubCmd::AddFromFile(s) => manager.subcmd_addfromfile(&s.file),
        //     SubCmd::Menu => manager.subcmd_menu(),
        // };

        // if manager.modified {
        //     if let Err(e) = manager.save_to_file(&path) {
        //         eprintln!("Failed to save to file: {}", e);
        //         return 1;
        //     }
        // }

        // code

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
