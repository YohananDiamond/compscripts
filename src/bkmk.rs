use clap::Clap;
use curl::easy::Easy;
use select::document::Document;
use select::node::Data;
use select::predicate::Name;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::path::Path;

mod lib;
use lib::array_serialization::*;
use lib::functions::{find_free_value, fzagnostic, touch_and_open, touch_read};
use lib::traits::DataManager;

fn main() {
    std::process::exit(BookmarkManager::start());
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Bookmark {
    id: u32,
    archived: bool,
    name: String,
    url: String,
    tags: Vec<String>,
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

struct BookmarkManager {
    data: Vec<Bookmark>,
    modified: bool,
    used_ids: HashSet<u32>,
}

impl DataManager for BookmarkManager {
    type Data = Bookmark;

    fn start() -> i32 {
        #[derive(Clap)]
        struct Opts {
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
        enum SubCmd {
            #[clap(about = "adds an URL to the bookmarks list")]
            Add(Add),
            #[clap(about = "adds the URLs from a newline-delimited bookmarks list file")]
            AddFromFile(AddFromFile),
            #[clap(about = "opens an interactive menu for managing bookmarks using fzagnostic")]
            Menu,
        }

        #[derive(Clap)]
        struct Add {
            pub url: String,
        }

        #[derive(Clap)]
        struct AddFromFile {
            pub file: String,
        }

        let options = Opts::parse();
        let path_string = match options.path {
            Some(cfg) => cfg,
            None => std::env::var("BKMK_FILE").unwrap_or(format!(
                "{}/.local/share/bkmk",
                std::env::var("HOME").unwrap()
            )),
        };
        let path = Path::new(&path_string);
        let contents = match touch_read(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to load bookmarks file: {}", e);
                return 1;
            }
        };

        let data: Vec<Bookmark> = match ArrayLines::import(&contents) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to parse bookmarks file: {}", e);
                return 1;
            }
        };

        let mut manager = match BookmarkManager::new(data) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to set up bookmarks: {}", e);
                return 1;
            }
        };

        let code: i32 = match options.subcmd {
            SubCmd::Add(s) => manager.subcmd_add(&s.url),
            SubCmd::AddFromFile(s) => manager.subcmd_addfromfile(&s.file),
            SubCmd::Menu => manager.subcmd_menu(),
        };

        if manager.modified {
            if let Err(e) = ArrayLines::new(&manager.data).save_to_file(&path) {
                eprintln!("Failed to save to file: {}", e);
                return 1;
            }
        }

        code
    }

    fn data_mut(&mut self) -> &mut Vec<Self::Data> {
        &mut self.data
    }

    fn data(&self) -> &Vec<Self::Data> {
        &self.data
    }
}

impl BookmarkManager {
    fn new(data: Vec<Bookmark>) -> Result<Self, String> {
        let mut used_ids: HashSet<u32> = HashSet::new();

        for bookmark in data.iter() {
            if used_ids.contains(&bookmark.id) {
                return Err(format!("repeated ID: {}", bookmark.id));
            } else {
                used_ids.insert(bookmark.id);
            }
        }

        Ok(BookmarkManager {
            data: data,
            modified: false,
            used_ids: used_ids,
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
        let mut file = match touch_and_open(path) {
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
            .data()
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
                    .data()
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
                self.data_mut()
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

    /// Returns a bool indicating whether the operation was successful
    fn add_bookmark_from_url(&mut self, url: &str) -> Result<(), u32> {
        let title = match url_get_title(url) {
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
        let result = self.add_bookmark(Bookmark {
            id: 0,
            archived: false,
            name: title,
            url: url.into(),
            tags: Vec::new(),
        });
        if result.is_ok() {
            eprintln!("added {}", bkfmt);
            self.modified = true;
        } else {
            eprintln!("failed to add {} (repeated URL)", bkfmt);
        }

        result
    }

    /// Note: the `id` field in `bookmark` is ignored.
    fn add_bookmark(&mut self, bookmark: Bookmark) -> Result<(), u32> {
        for b in self.data() {
            if b.url == bookmark.url {
                return Err(b.id);
            }
        }

        let id = find_free_value(&self.used_ids);
        self.data_mut().push(Bookmark {
            id: id,
            archived: false,
            ..bookmark
        });
        self.used_ids.insert(id);

        Ok(())
    }
}

fn url_get_title(url: &str) -> Result<String, String> {
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

    // Parse the HTTP response code
    let response_code = easy.response_code().unwrap();
    match response_code {
        300..=399 => return Err(format!("got redirection code {}", response_code)),
        400..=499 => return Err(format!("got client error code {}", response_code)),
        500..=599 => return Err(format!("got server error code {}", response_code)),
        _ => (),
    }

    let document = match Document::from_read(String::from_utf8_lossy(&vec).as_bytes()) {
        Ok(doc) => doc,
        Err(err) => return Err(format!("IO Error: {}", err)),
    };

    let titles: Vec<_> = document.find(Name("title")).collect();

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
