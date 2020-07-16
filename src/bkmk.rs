use clap::Clap;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

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
        transfer.perform().unwrap();
    }

    // parse the HTTP response code
    let c = easy.response_code().unwrap();
    match c {
        300..=399 => return Err(format!("Redirection code {}", c)),
        400..=499 => return Err(format!("Client error {}", c)),
        500..=599 => return Err(format!("Server error {}", c)),
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
        let title = match get_webpage_title(url) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Failed to get title: {}", e);
                eprint!("New title: ");
                io::stdout().flush().expect("failed to flush stdout");

                let mut buffer = String::new();
                io::stdin()
                    .read_line(&mut buffer)
                    .expect("failed to read line");

                buffer
            }
        }.trim().to_string();

        let bookmark = Bookmark {
            id: {
                let max = self.used_ids.iter().fold(0, |total, item| total.max(*item));
                if self.used_ids.contains(&max) {
                    max + 1
                } else {
                    max
                }
            },
            archived: false,
            name: title,
            url: url.to_string(),
            tags: Vec::new(),
        };

        self.bookmarks.push(bookmark);
        self.modified = true;

        0
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
