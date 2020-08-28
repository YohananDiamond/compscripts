mod bookmark;
mod cli;
mod manager;

use clap::Clap;
use std::path::Path;
use std::io::{Read, Write};
use std::process::Command;

use bookmark::Bookmark;
use cli::*;
use core::aliases::getenv;
use core::data::{JsonSerializer, Manager};
use core::misc::{ExitResult, ExitCode};
use manager::BookmarkManager;

fn main() -> ExitCode {
    let fallback_file = format!("{HOME}/.local/share/bkmk", HOME=getenv("HOME").unwrap());
    let bkmk_file = match getenv("BKMK_FILE") {
        Ok(s) if s.len() == 0 => fallback_file,
        Err(_) => fallback_file,
        Ok(other) => other,
    };

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
        Err(e) => return ExitCode::from(Err(e)),
    };

    let code = match options.subcmd {
        SubCmd::Add(param) => subcmd::add(&mut manager, param).into(),
        SubCmd::AddFromFile(param) => subcmd::add_from_file(&mut manager, param).into(),
        SubCmd::Menu => subcmd::menu(&mut manager).into(),
    };

    match code {
        ExitCode(0) => (),
        other => return other,
    }

    if let Err(e) = manager.save_if_modified(&path) {
        eprintln!("Failed to save changes to file: {}", e);
        ExitCode(1)
    } else {
        ExitCode(0)
    }
}

mod subcmd {
    use super::*;
    use core::misc::fzagnostic;

    pub fn add(manager: &mut BookmarkManager, param: AddParameters) -> Result<(), String> {
        if let Some(title) = param.title {
            manager.add_bookmark(title, param.url, Vec::new())
        } else {
            manager.add_bookmark_from_url(param.url, true)
        }
    }

    pub fn add_from_file(
        manager: &mut BookmarkManager,
        param: FileParameters,
    ) -> Result<(), String> {
        let path = Path::new(&param.file);
        let mut file = match core::io::touch_and_open(path) {
            Ok(file) => file,
            Err(e) => return Err(format!("failed to open file: {}", e)),
        };

        let mut contents = String::new();
        if let Err(e) = file.read_to_string(&mut contents) {
            return Err(format!("failed to read file: {}", e));
        }

        for line in contents.split("\n") {
            if line.trim().len() != 0 {
                manager.add_bookmark_from_url(line.to_string(), true)?;
            }
        }

        Ok(())
    }

    pub fn menu(manager: &mut BookmarkManager) -> ExitResult {
        let not_archived: Vec<&Bookmark> = manager.data().iter()
            .filter(|b| !b.archived)
            .collect();

        if not_archived.len() == 0 {
            return ExitResult::from(format!("There are no unarchived bookmarks to select"));
        }

        let chosen_id = {
            // TODO: align selection numbers
            let input = not_archived.iter()
                .enumerate()
                .map(|(i, b)| format!("{:>3} {:<95} ({})", i, b.name, b.url))
                .collect::<Vec<String>>()
                .join("\n");

            match fzagnostic(
                &format!("Bookmark ({}):", not_archived.len()),
                &input, 30,
            ) {
                Some(s) => {
                    not_archived[s.split(" ").next().unwrap().parse::<usize>().unwrap()].id
                },
                None => return ExitResult::SilentError,
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

            match core::misc::fzagnostic("Action:", &input, 30) {
                Some(s) => s.split(" ").collect::<Vec<&str>>()[0].parse::<usize>().unwrap(),
                None => return ExitResult::SilentError,
            }
        };

        match chosen_action {
            0 => {
                manager.interact(chosen_id, |b| {
                    let opener = getenv("OPENER").unwrap_or("xdg-open".into());

                    match Command::new(opener)
                        .args(&[&b.url])
                        .spawn()
                    {
                        Ok(mut child) => match child.wait().unwrap().code().unwrap() {
                            0 => ExitResult::Success,
                            _ => ExitResult::SilentError,
                        },
                        Err(_) => ExitResult::from("failed to start opener command"),
                    }

                }).unwrap()
            },
            1 => {
                manager.interact_mut(chosen_id, |b| {
                    b.archived = true;

                    ExitResult::Success
                }).unwrap()
            },
            2 => {
                manager.interact_mut(chosen_id, |b| {
                    match Command::new("xclip")
                        .args(&["-sel", "clipboard"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                    {
                        Ok(mut child) => {
                            let stdin = child.stdin.as_mut().unwrap();
                            write!(stdin, "{}", b.url).unwrap();

                            if child.wait().unwrap().code().unwrap() == 0 {
                                ExitResult::Success
                            } else {
                                ExitResult::from("failed to save to clipboard")
                            }
                        },
                        Err(_) => {
                            ExitResult::from("failed to start xclip command")
                        }
                    }
                }).unwrap()
            },
            _ => panic!("unknown code"), // TODO: turn this into a not-panic, but just a simple error
        }
    }
}
