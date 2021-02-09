use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

use clap::Clap;

mod cli;
use cli::*;

mod bookmark;
use bookmark::Bookmark;

mod manager;
use manager::BookmarkManager;

use utils::aliases::getenv;
use utils::data::{JsonSerializer, Manager};
use utils::error::{CliResult, ExitCode};
use utils::misc::fzagnostic;

fn fallback_string_if_needed<'a>(string: &'a str) -> &'a str {
    for ch in string.chars() {
        if !matches!(ch, '\n' | ' ' | '\t' | '\r') {
            return string;
        }
    }

    "[]"
}

fn main() -> ExitCode {
    let home = getenv("HOME").expect("HOME directory is unset - it is needed");

    let _cache_dir: String = std::env::var("XDG_CACHE_DIR")
        .ok()
        .unwrap_or_else(|| format!("{}/.cache", home));

    let data_dir: String = std::env::var("XDG_DATA_HOME")
        .ok()
        .or_else(|| std::env::var("XDG_DATA_DIR").ok())
        .unwrap_or_else(|| format!("{}/.local/share", home));

    let fallback_file = format!("{}/bkmk", data_dir);

    let bkmk_file = match std::env::var("BKMK_FILE") {
        Err(_) => fallback_file,
        Ok(var) if var.len() == 0 => fallback_file,
        Ok(var) => var,
    };

    let options = cli::Options::parse();

    // try blocks :))
    (|| -> CliResult {
        let path_string = options.path.unwrap_or(bkmk_file);
        let path = Path::new(&path_string);

        let contents = utils::io::touch_read(&path).or_else(|why| {
            CliResult::display_err(format!("Failed to load file: {}", why)).into()
        })?;

        let new_contents = fallback_string_if_needed(&contents);

        let data: Vec<Bookmark> = BookmarkManager::import(new_contents).or_else(|why| {
            CliResult::display_err(format!("Failed to parse file: {}", why)).into()
        })?;

        let mut manager =
            BookmarkManager::new(data).or_else(|err| CliResult::display_err(err).into())?;

        match options.subcmd {
            SubCmd::Add(param) => subcmd_add(&mut manager, param),
            SubCmd::AddFromFile(param) => subcmd_add_from_file(&mut manager, param),
            SubCmd::Menu => subcmd_menu(&mut manager),
        }?;

        manager.save_if_modified(&path).or_else(|why| {
            CliResult::display_err(format!("Failed to save changes to file: {}", why)).into()
        })?;

        CliResult::EMPTY_OK
    })()
    .process()
}

pub fn subcmd_add(manager: &mut BookmarkManager, param: AddParameters) -> CliResult {
    CliResult::from_display_result(if let Some(title) = param.title {
        manager.add_bookmark(title, param.url, Vec::new())
    } else {
        manager.add_bookmark_from_url(param.url, true)
    })
}

pub fn subcmd_add_from_file(manager: &mut BookmarkManager, param: FileParameters) -> CliResult {
    let path = Path::new(&param.file);
    let mut file = match utils::io::touch_and_open(path) {
        Ok(file) => file,
        Err(e) => return CliResult::display_err(format!("failed to open file: {}", e)),
    };

    let contents = {
        let mut s = String::new();
        match file.read_to_string(&mut s) {
            Ok(_) => s,
            Err(e) => return CliResult::display_err(format!("failed to read file: {}", e)),
        }
    };

    for url in contents
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Err(e) = manager.add_bookmark_from_url(url.into(), true) {
            return CliResult::display_err(e);
        }
    }

    CliResult::EMPTY_OK
}

pub fn subcmd_menu(manager: &mut BookmarkManager) -> CliResult {
    let not_archived: Vec<&Bookmark> = manager
        .data()
        .iter()
        .filter(|bkmk| !bkmk.archived)
        .collect();

    if not_archived.len() == 0 {
        return CliResult::display_err(format!("There are no unarchived bookmarks to select"));
    }

    let chosen_id = {
        match fzagnostic(
            &format!("Bookmark ({}):", not_archived.len()),
            not_archived
                .iter()
                .enumerate()
                .map(|(i, bkmk)| format!("{:>3} {:<95} ({})", i, bkmk.name, bkmk.url)),
            30,
        ) {
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
            Err(err) => return CliResult { inner: Err(err) },
        }
    };

    type ActionSig = fn(&mut BookmarkManager, u32) -> CliResult;

    static ACTIONS: [(&str, ActionSig); 5] = [
        ("open (via $OPENER || xdg-open)", |manager, id| {
            manager
                .interact(id, |bkmk| {
                    let opener = getenv("OPENER").unwrap_or("xdg-open".into());

                    match Command::new(opener).args(&[&bkmk.url]).spawn() {
                        Ok(mut child) => match child.wait().unwrap().code().unwrap() {
                            0 => CliResult::EMPTY_OK,
                            _ => CliResult::silent_err(),
                        },
                        Err(why) => CliResult::display_err(format!(
                            "failed to start opener command: {}",
                            why
                        )),
                    }
                })
                .unwrap()
        }),
        ("archive", |manager, id| {
            manager
                .interact_mut(id, |bkmk| {
                    bkmk.archived = true;

                    CliResult::EMPTY_OK
                })
                .unwrap()
        }),
        ("copy to clipboard (via xclip)", |manager, id| {
            manager
                .interact_mut(id, |bkmk| {
                    match Command::new("xclip")
                        .args(&["-sel", "clipboard"])
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                    {
                        Ok(mut child) => {
                            let stdin = child.stdin.as_mut().unwrap();
                            write!(stdin, "{}", bkmk.url).unwrap();

                            if child.wait().unwrap().code().unwrap() == 0 {
                                CliResult::EMPTY_OK
                            } else {
                                CliResult::display_err("failed to save to clipboard")
                            }
                        }
                        Err(why) => CliResult::display_err(format!(
                            "failed to start xclip command: {}",
                            why
                        )),
                    }
                })
                .unwrap()
        }),
        ("delete", |manager, id| {
            let pos = manager
                .data()
                .iter()
                .position(|bkmk| bkmk.id == id)
                .unwrap();
            manager.data_mut().swap_remove(pos);
            manager.after_interact_mut_hook();

            CliResult::EMPTY_OK
        }),
        ("edit title", |manager, id| {
            manager
                .interact_mut(id, |bkmk| {
                    match utils::tmp::edit_text(&bkmk.name, Some("txt")) {
                        Ok((new_title, 0)) => {
                            let new_title = new_title
                                .trim()
                                .chars()
                                .filter(|c| !matches!(c, '\n' | '\r'))
                                .collect::<String>();

                            bkmk.name = new_title;

                            CliResult::EMPTY_OK
                        }
                        Ok((_, _)) => CliResult::silent_err(),
                        Err(why) => {
                            CliResult::display_err(format!("Failed to edit title: {}", why))
                        }
                    }
                })
                .unwrap()
        }),
    ];

    let action_id = {
        match fzagnostic(
            "Action:",
            ACTIONS
                .iter()
                .enumerate()
                .map(|(i, (name, _))| format!("{} {}", i, name)),
            30,
        ) {
            Ok(s) => s.split(" ").nth(0).unwrap().parse::<usize>().unwrap(),
            Err(err) => return CliResult { inner: Err(err) },
        }
    };

    match ACTIONS.get(action_id) {
        Some((_, func)) => func(manager, chosen_id),
        None => CliResult::display_err(format!("Invalid action ID: {}", action_id)),
    }
}
