pub mod lib;

use clap::Clap;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use lib::fzagnostic;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Bookmark {
    id: u32,
    archived: bool,
    name: String,
    url: String,
    tags: Vec<String>,
}

struct BookmarkManager {
    pub bookmarks: Vec<Bookmark>,
    pub modified: bool,
    used_ids: HashSet<u32>,
}

enum Error {
    JsonError(serde_json::error::Error),
    RepeatedID(u32),
}

fn main_() -> i32 {
    use argparse::*;

    let options = Opts::parse();
    let path_str = match options.path {
        Some(cfg) => cfg,
        None => std::env::var("BKMK_FILE").unwrap_or(format!(
            "{}/.local/share/bkmk",
            std::env::var("HOME").unwrap()
        )),
    };
    let path = Path::new(&path_str);

    let contents = {
        let mut file = match open_file(path, true) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("failed to create file: {}", e);
                return 1;
            }
        };

        // read contents of the file
        let mut contents = String::new();
        if let Err(_) = file.read_to_string(&mut contents) {
            eprintln!("failed to read file buffer");
            return 1;
        } else {
            contents
        }
    };

    let mut manager = match BookmarkManager::from_json_lines(&contents) {
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

    let code: i32 = match options.subcmd {
        SubCmd::Add(s) => manager.subcmd_add(&s.url),
        SubCmd::AddFromFile(s) => manager.subcmd_addfromfile(&s.file),
        SubCmd::Menu => manager.subcmd_menu(),
    };

    if manager.modified {
        if let Err(_) = manager.save_to_file(&path) {
            eprintln!("failed to save to file");
            return 1;
        }
    }

    code
}

fn open_file(path: &Path, create_if_needed: bool) -> Result<File, String> {
    if path.exists() {
        if path.is_dir() {
            Err("path is a directory".into())
        } else {
            match File::open(path) {
                Ok(f) => Ok(f),
                Err(e) => Err(format!("{}", e)),
            }
        }
    } else {
        if create_if_needed {
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    if parent.is_file() {
                        return Err(format!(
                            "parent path {} is not a directory",
                            parent.display()
                        ));
                    }
                } else {
                    if let Err(e) = std::fs::create_dir(parent) {
                        return Err(format!(
                            "failed to create parent path {}: {}",
                            parent.display(),
                            e
                        ));
                    }
                }
            }

            match File::create(path) {
                Ok(_) => match File::open(path) {
                    Ok(f) => Ok(f),
                    Err(e) => Err(format!("{}", e)),
                },
                Err(e) => Err(format!("failed to create file: {}", e)),
            }
        } else {
            Err("file does not exist".into())
        }
    }
}

mod argparse {
    use super::*;

    #[derive(Clap)]
    pub struct Opts {
        #[clap(
            short,
            long,
            about = "the path to the bookmarks file (default: $BKMK_FILE -> ~/.local/share/bkmk)"
        )]
        pub path: Option<String>,
        #[clap(subcommand)]
        pub subcmd: SubCmd,
    }

    #[derive(Clap)]
    pub enum SubCmd {
        #[clap(about = "adds an URL to the bookmarks list")]
        Add(Add),
        #[clap(about = "adds the URLs from a newline-delimited bookmarks list file")]
        AddFromFile(AddFromFile),
        #[clap(about = "opens an interactive menu for managing bookmarks using fzagnostic")]
        Menu,
    }

    #[derive(Clap)]
    pub struct Add {
        pub url: String,
    }

    #[derive(Clap)]
    pub struct AddFromFile {
        pub file: String,
    }
}

fn get_webpage_title(url: &str) -> Result<String, String> {
    use select::document::Document;
    use select::node::Data;
    use select::predicate::Name;

    use curl::easy::Easy;
    // use std::io::stdout;
    // use std::io::Write;

    let mut vec = Vec::new();
    let mut easy = Easy::new();
    easy.url(url).expect("failed to set url");

    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                vec.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();

        let _ = transfer.perform();
    }

    // parse the HTTP response code
    let c = easy.response_code().unwrap();
    match c {
        300..=399 => return Err(format!("got redirection code {}", c)),
        400..=499 => return Err(format!("got client error code {}", c)),
        500..=599 => return Err(format!("got server error code {}", c)),
        _ => (),
    }

    // convert vec to string
    let string: String = String::from_utf8_lossy(&vec).to_string();

    let document = match Document::from_read(string.as_bytes()) {
        Ok(doc) => doc,
        Err(err) => return Err(format!("IO Error: {}", err)),
    };

    let titles = document.find(Name("title")).collect::<Vec<_>>();

    if titles.len() == 0 {
        Err(String::from("Couldn't find any <title> tags in page"))
    } else {
        // get the first title tag, ignore the rest
        let children = titles[0]
            .children()
            .filter(|x| match x.data() {
                Data::Text(_) => true,
                _ => false,
            })
            .collect::<Vec<_>>();

        if children.len() == 0 {
            Err(String::from("Empty <title> tag found"))
        } else {
            // there's no unwrap here, so I guess I need to use this syntax.
            Ok(String::from(children[0].as_text().unwrap()))
        }
    }
}

impl BookmarkManager {
    pub fn from_json_lines(lines: &str) -> Result<BookmarkManager, Error> {
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
            used_ids,
            modified: false,
        })
    }

    pub fn subcmd_add(&mut self, url: &str) -> i32 {
        if self.add_bookmark_from_url(url).is_ok() {
            0
        } else {
            1
        }
    }

    pub fn subcmd_addfromfile(&mut self, file: &str) -> i32 {
        let path = Path::new(file);
        let mut file = match open_file(path, false) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("failed to open file: {}", e);
                return 1;
            }
        };

        let mut contents = String::new();
        if let Err(_) = file.read_to_string(&mut contents) {
            eprintln!("failed to read file buffer");
            return 1;
        }

        let mut len: usize = 0;
        let mut successful: usize = 0;
        for line in contents.split("\n") {
            if line.trim().len() != 0 {
                len += 1;
                if self.add_bookmark_from_url(line).is_ok() {
                    successful += 1;
                }
            }
        }

        eprintln!("added {} bookmarks out of {} urls", successful, len);

        // exit gracefully if at least one bookmark was written
        if successful != 0 {
            0
        } else {
            1
        }
    }

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
                self.modified = true;
                0
            }
            _ => panic!("invalid code"),
        }
    }

    pub fn save_to_file(&mut self, file: &Path) -> Result<(), ()> {
        self.bookmarks.sort();
        let compiled_string = self
            .bookmarks
            .iter()
            .map(|x| serde_json::to_string(x).unwrap())
            .collect::<Vec<String>>()
            .join("\n");
        match std::fs::write(file, &compiled_string) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    /// Returns a bool indicating whether the operation was successful
    fn add_bookmark_from_url(&mut self, url: &str) -> Result<(), u32> {
        let title = match get_webpage_title(url) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to get title: {}", e);
                eprintln!("Url: {:?}", url);
                eprint!("Type a new title: ");
                io::stdout().flush().expect("failed to flush stdout");

                let mut buffer = String::new();
                io::stdin()
                    .read_line(&mut buffer)
                    .expect("failed to read line");

                buffer
            }
        }
        .trim()
        .to_string();

        let bkfmt = format!("@bookmark[url: {:?}, name: {:?}]", url, title);
        let result = self.add_bookmark(title, url.into(), Vec::new());
        if result.is_ok() {
            eprintln!("added {}", bkfmt);
            self.modified = true;
        } else {
            eprintln!("failed to add {} (repeated URL)", bkfmt);
        }

        result
    }

    /// Returns a bool indicating whether the operation was successful
    fn add_bookmark(&mut self, name: String, url: String, tags: Vec<String>) -> Result<(), u32> {
        for bookmark in &self.bookmarks {
            if bookmark.url == url {
                return Err(bookmark.id);
            }
        }

        let id: u32 = {
            let max = self.used_ids.iter().fold(0, |total, item| total.max(*item));
            if self.used_ids.contains(&max) {
                max + 1
            } else {
                max
            }
        };
        self.bookmarks.push(Bookmark {
            id,
            archived: false,
            name,
            url,
            tags,
        });
        self.used_ids.insert(id);

        Ok(())
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

fn main() { std::process::exit(main_()); }
