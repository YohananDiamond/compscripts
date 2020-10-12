mod bookmark;
mod cli;
mod manager;

use clap::Clap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

use bookmark::Bookmark;
use cli::*;
use core::aliases::getenv;
use core::data::{JsonSerializer, Manager};
use core::error::{ExitCode, ExitResult};
use manager::BookmarkManager;

fn main() -> ExitCode {
    let home = getenv("HOME").expect("HOME directory is unset - it is needed");

    let cache_dir: String = std::env::var("XDG_CACHE_DIR")
        .ok()
        .or_else(|| Some(format!("{}/.cache", home)))
        .unwrap();
    let data_dir: String = std::env::var("XDG_DATA_HOME")
        .ok()
        .or_else(|| std::env::var("XDG_DATA_DIR").ok())
        .or_else(|| Some(format!("{}/.local/share", home)))
        .unwrap();

    let fallback_file = format!("{}/bkmk", data_dir);

    let bkmk_file = match std::env::var("BKMK_FILE") {
        Err(_) => fallback_file,
        Ok(var) if var.len() == 0 => fallback_file,
        Ok(var) => var,
    };

    let _mutex_file = format!("{}/bkmk-mutex", cache_dir);

    let options = cli::Options::parse();

    let pstring = match options.path {
        Some(cfg) => cfg,
        None => bkmk_file,
    };

    let path = Path::new(&pstring);
    let contents = match core::io::touch_read(&path) {
        Ok(s) => {
            if s.chars()
                .filter(|c| !matches!(c, '\n' | ' ' | '\t'))
                .count()
                == 0
            {
                String::from("[]")
            } else {
                s
            }
        }
        Err(e) => {
            eprintln!("Failed to load file: {}", e);
            return ExitResult::from(format!("failed to load file")).into();
        }
    };

    let data: Vec<Bookmark> = match BookmarkManager::import(&contents) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Failed to parse file: {}", e);
            return ExitCode(1);
        }
    };

    let mut manager = match BookmarkManager::new(data) {
        Ok(m) => m,
        Err(e) => return ExitResult::from(e).into(),
    };

    let result = match options.subcmd {
        SubCmd::Add(param) => subcmd::add(&mut manager, param),
        SubCmd::AddFromFile(param) => subcmd::add_from_file(&mut manager, param),
        SubCmd::Menu => subcmd::menu(&mut manager),
    };

    ExitCode::from(result).and_then(|| {
        if let Err(e) = manager.save_if_modified(&path) {
            eprintln!("Failed to save changes to file: {}", e);
            ExitCode(1)
        } else {
            ExitCode(0)
        }
    })
}

mod subcmd {
    use super::*;
    use core::misc::fzagnostic;

    pub fn add(manager: &mut BookmarkManager, param: AddParameters) -> ExitResult {
        ExitResult::from_display_result(if let Some(title) = param.title {
            manager.add_bookmark(title, param.url, Vec::new())
        } else {
            manager.add_bookmark_from_url(param.url, true)
        })
    }

    pub fn add_from_file(manager: &mut BookmarkManager, param: FileParameters) -> ExitResult {
        let path = Path::new(&param.file);
        let mut file = match core::io::touch_and_open(path) {
            Ok(file) => file,
            Err(e) => return ExitResult::from(format!("failed to open file: {}", e)),
        };

        let mut contents = String::new();
        if let Err(e) = file.read_to_string(&mut contents) {
            return ExitResult::from(format!("failed to read file: {}", e));
        }

        for line in contents.split("\n") {
            if line.trim().len() != 0 {
                if let Err(e) = manager.add_bookmark_from_url(line.to_string(), true) {
                    return ExitResult::from(e);
                }
            }
        }

        ExitResult::Ok
    }

    pub fn menu(manager: &mut BookmarkManager) -> ExitResult {
        let not_archived: Vec<&Bookmark> = manager.data().iter().filter(|b| !b.archived).collect();

        if not_archived.len() == 0 {
            return ExitResult::from(format!("There are no unarchived bookmarks to select"));
        }

        let chosen_id = {
            // TODO: align selection numbers
            let input = not_archived
                .iter()
                .enumerate()
                .map(|(i, b)| format!("{:>3} {:<95} ({})", i, b.name, b.url))
                .collect::<Vec<String>>()
                .join("\n");

            match fzagnostic(&format!("Bookmark ({}):", not_archived.len()), &input, 30) {
                Ok(s) => {
                    not_archived[s
                        .trim()
                        .split(" ")
                        .next()
                        .unwrap()
                        .parse::<usize>()
                        .unwrap()]
                    .id
                }
                Err(e) if e == "" => return ExitResult::SilentErr,
                Err(e) => return ExitResult::from(e),
            }
        };

        const ACTIONS: &'static [&'static str] = &[
            "open (via $OPENER -> xdg-open)",
            "archive",
            "copy (via xclip)",
        ];

        let chosen_action = {
            let input = ACTIONS
                .iter()
                .enumerate()
                .map(|(i, a)| format!("{} {}", i, a))
                .collect::<Vec<String>>()
                .join("\n");

            match fzagnostic("Action:", &input, 30) {
                Ok(s) => s.split(" ").collect::<Vec<&str>>()[0]
                    .parse::<usize>()
                    .unwrap(),
                Err(e) if e == "" => return ExitResult::SilentErr,
                Err(e) => return ExitResult::from(e),
            }
        };

        match chosen_action {
            0 => manager
                .interact(chosen_id, |b| {
                    let opener = getenv("OPENER").unwrap_or("xdg-open".into());

                    match Command::new(opener).args(&[&b.url]).spawn() {
                        Ok(mut child) => match child.wait().unwrap().code().unwrap() {
                            0 => ExitResult::Ok,
                            _ => ExitResult::SilentErr,
                        },
                        Err(_) => ExitResult::from("failed to start opener command"),
                    }
                })
                .unwrap(),
            1 => manager
                .interact_mut(chosen_id, |b| {
                    b.archived = true;

                    ExitResult::Ok
                })
                .unwrap(),
            2 => manager
                .interact_mut(chosen_id, |b| {
                    match Command::new("xclip")
                        .args(&["-sel", "clipboard"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                    {
                        Ok(mut child) => {
                            let stdin = child.stdin.as_mut().unwrap();
                            write!(stdin, "{}", b.url).unwrap();

                            if child.wait().unwrap().code().unwrap() == 0 {
                                ExitResult::Ok
                            } else {
                                ExitResult::from("failed to save to clipboard")
                            }
                        }
                        Err(_) => ExitResult::from("failed to start xclip command"),
                    }
                })
                .unwrap(),
            _ => panic!("unknown code"), // TODO: turn this into a not-panic, but just a simple error
        }
    }
}
