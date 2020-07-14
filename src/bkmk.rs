use clap::Clap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::cmp::Ordering;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Bookmark {
    id: u32,
    archived: bool,
    name: String,
    url: String,
    tags: Vec<String>,
}

struct BookmarkManager<'a> {
    pub bookmarks: Vec<Bookmark>,
    path: &'a Path,
    used_ids: HashSet<u32>,
}

enum Error {
    JsonError(serde_json::error::Error),
    RepeatedID(u32),
}

fn main() {
    std::process::exit(app());
}

fn app() -> i32 {
    use argparse::*;

    let options = Opts::parse();
    let path_str = match options.path {
        Some(cfg) => cfg,
        None => format!("{}/.local/share/bkmk", std::env::var("HOME").unwrap()), // might crash on some alien system
    };
    let path = Path::new(&path_str);

    let contents = {
        let mut file = if path.exists() {
            // if path is dir
            if path.is_dir() {
                eprintln!(
                    "bookmarks path {} is a directory; could not continue",
                    path.display()
                );
                return 1;
            } else {
                match File::open(path) {
                    Ok(f) => f,
                    Err(_) => {
                        eprintln!("failed to open file; aborting");
                        return 1;
                    }
                }
            }
        } else {
            // create path if it doesn't exist
            eprintln!(
                "bookmarks path {} doesn't exist; creating it...",
                path.display()
            );

            let mut ancestors = path.ancestors();
            ancestors.next().unwrap();
            let parent = ancestors.next().unwrap();

            if parent.exists() {
                if !parent.is_dir() {
                    eprintln!("parent path {} is not a directory", parent.display());
                    return 1;
                }
            } else {
                if let Err(_) = std::fs::create_dir(parent) {
                    eprintln!("failed to create dir {}; aborting", parent.display());
                    return 1;
                };
            }

            match File::create(path) {
                Ok(_) => match File::open(path) {
                    Ok(f) => f,
                    Err(_) => {
                        eprintln!("failed to open file; aborting");
                        return 1;
                    }
                },
                Err(_) => {
                    eprintln!("failed to create file; aborting");
                    return 1;
                }
            }
        };

        // read contents of the file
        let mut contents = String::new();
        if let Err(_) = file.read_to_string(&mut contents) {
            eprintln!("failed to read file; aborting");
            return 1;
        } else {
            contents
        }
    };

    let mut manager = match BookmarkManager::from_json_lines(&contents, &path) {
        Ok(m) => m,
        Err(e) => match e {
            Error::JsonError(e) => {
                eprintln!("failed to parse bookmarks file: {}", e);
                return 1;
            }
            Error::RepeatedID(id) => {
                eprintln!(
                    "repeated ID {} in bookmarks file; this will have to be fixed manually",
                    id
                );
                return 1;
            }
        },
    };

    return match options.subcmd {
        // SubCmd::Add(url) => manager.subcmd_add(url),
        SubCmd::Menu => manager.subcmd_menu(),
        _ => 99,
    };
}

mod argparse {
    use super::*;

    #[derive(Clap)]
    pub struct Opts {
        #[clap(
            short,
            long,
            about = "the path to the bookmarks file (default: ~/.local/share/bkmk)"
        )]
        pub path: Option<String>,
        #[clap(subcommand)]
        pub subcmd: SubCmd,
    }

    #[derive(Clap)]
    pub enum SubCmd {
        #[clap(about = "adds an URL to the bookmarks list")]
        Add(Add),
        Menu,
    }

    #[derive(Clap)]
    pub struct Add {
        pub url: String,
    }
}

fn fzagnostic(prompt: &str, input: &str, height: u32) -> Option<String> {
    match std::process::Command::new("fzagnostic")
        .args(&["-h", &format!("{}", height), "-p", prompt])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            let stdin = child.stdin.as_mut().unwrap();
            write!(stdin, "{}", input).unwrap();

            if child.wait().unwrap().code().unwrap() == 0 {
                let mut choice = String::new();
                match child.stdout.as_mut().unwrap().read_to_string(&mut choice) {
                    Ok(_) => Some(choice),
                    Err(_) => None,
                }
            } else {
                None
            }
        }
        Err(_) => {
            eprintln!("failed to run command");
            None
        }
    }
}

impl BookmarkManager<'_> {
    pub fn from_json_lines<'a>(lines: &str, path: &'a Path) -> Result<BookmarkManager<'a>, Error> {
        let mut used_ids: HashSet<u32> = HashSet::new();

        // get IDs for bookmarks
        let bookmarks: Vec<Bookmark> = match lines
            .split("\n")
            .filter_map(|line| {
                if line.len() == 0 {
                    None
                } else {
                    Some(serde_json::from_str(line))
                }
            })
            .collect::<Result<Vec<Bookmark>, serde_json::error::Error>>()
        {
            Ok(v) => v,
            Err(e) => return Err(Error::JsonError(e)),
        };

        // check for repeated IDs
        for bookmark in bookmarks.iter() {
            if used_ids.contains(&bookmark.id) {
                return Err(Error::RepeatedID(bookmark.id));
            } else {
                used_ids.insert(bookmark.id);
            }
        }

        Ok(BookmarkManager {
            bookmarks,
            path,
            used_ids,
        })
    }

    // pub fn subcmd_add(&mut self, url: &str) -> i32;
    pub fn subcmd_menu(&mut self) -> i32 {
        let non_archived: Vec<(usize, &Bookmark)> = self
            .bookmarks
            .iter()
            .filter(|b| !b.archived)
            .enumerate()
            .collect();

        if non_archived.len() == 0 {
            eprintln!("there are no unarchived bookmarks to select");
            return 1;
        }

        let chosen_bookmark_id: u32 = {
            let input = non_archived
                .iter()
                .map(|(i, b)| format!("{} {}", i, b.name))
                .collect::<Vec<String>>()
                .join("\n");
            match fzagnostic(&format!("Bookmark ({}):", non_archived.len()), &input, 30) {
                Some(s) => {
                    non_archived[s.split(" ").collect::<Vec<&str>>()[0]
                        .parse::<usize>()
                        .unwrap()]
                    .1
                    .id
                }
                None => return 1,
            }
        };

        let actions = vec!["open (via $OPENER)", "archive"];

        let chosen_action = {
            let input = actions
                .iter()
                .enumerate()
                .map(|(i, a)| format!("{} {}", i, a))
                .collect::<Vec<String>>()
                .join("\n");
            match fzagnostic("Action:", &input, 30) {
                Some(s) => s.split(" ").collect::<Vec<&str>>()[0]
                    .parse::<usize>()
                    .unwrap(),
                None => return 1,
            }
        };

        match chosen_action {
            0 => {
                match std::process::Command::new(
                    std::env::var("OPENER").unwrap_or("xdg-opener".into()),
                )
                .args(&[self
                    .bookmarks
                    .iter()
                    .find(|b| b.id == chosen_bookmark_id)
                    .unwrap()
                    .url
                    .as_str()])
                .spawn()
                {
                    Ok(mut child) => child.wait().unwrap().code().unwrap(),
                    Err(_) => {
                        eprintln!("failed to start opener command");
                        1
                    }
                }
            }
            1 => {
                self.bookmarks
                    .iter_mut()
                    .find(|b| b.id == chosen_bookmark_id)
                    .unwrap()
                    .archived = true;
                match self.save_to_file() {
                    Ok(()) => 0,
                    Err(()) => {
                        println!("failed to save to file");
                        1
                    }
                }
            }
            _ => panic!("invalid code"),
        }
    }

    pub fn save_to_file(&mut self) -> Result<(), ()> {
        self.bookmarks.sort();
        let compiled_string = self
            .bookmarks
            .iter()
            .map(|x| serde_json::to_string(x).unwrap())
            .collect::<Vec<String>>()
            .join("\n");
        match std::fs::write(self.path, &compiled_string) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
}

impl Ord for Bookmark {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Bookmark {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
