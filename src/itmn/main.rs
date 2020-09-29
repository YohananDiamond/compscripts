#![feature(termination_trait_lib)]

use clap::Clap;
use std::path::Path;

mod cli;
mod item;
mod manager;

use cli::*;
use core::aliases::getenv;
use core::data::{JsonSerializer, Manager};
use core::error::ExitCode;
use item::{Item, State};
use manager::{Error, ItemManager};
use report::ReportStyle;

mod report {
    use super::{Item, State};

    #[derive(Clone, Copy)]
    pub enum ReportStyle {
        Shallow,
        Brief,
        Tree,
    }

    // pub mod generators {}

    pub fn print_single_item<F: Fn(&Item) -> bool + Copy>(item: &Item, indentation: usize, f: F) {
        if f(item) {
            eprintln!(
                "{}{} [{:>02}]{} {}",
                std::iter::repeat(' ')
                    .take(indentation * 2)
                    .collect::<String>(),
                match item.state {
                    State::Todo => 'o',
                    State::Done => 'x',
                    State::Note => '-',
                },
                item.ref_id.unwrap_or(item.internal_id),
                match &item.context {
                    Some(c) => format!(" @{}", c),
                    None => String::new(),
                },
                item.name,
            );
        }
    }

    pub fn print_item_styled<F: Fn(&Item) -> bool + Copy>(
        item: &Item,
        style: ReportStyle,
        indentation: usize,
        f: F,
    ) {
        let f_result = f(item);

        match style {
            ReportStyle::Shallow => {
                print_single_item(item, indentation, f);
            }
            ReportStyle::Brief => {
                print_single_item(item, indentation, f);

                if f_result {
                    if !item.children.is_empty() {
                        print_item_styled(
                            &item.children[0],
                            ReportStyle::Shallow,
                            indentation + 1,
                            f,
                        );
                    }
                    if item.children.len() > 1 {
                        eprintln!(
                            "{}  {} more...",
                            std::iter::repeat(' ')
                                .take(indentation * 2)
                                .collect::<String>(),
                            item.children.len() - 1
                        );
                    }
                }
            }
            ReportStyle::Tree => {
                print_single_item(item, indentation, f);

                if f_result {
                    for child in &item.children {
                        print_item_styled(&child, ReportStyle::Tree, indentation + 1, f);
                    }
                }
            }
        }
    }

    // TODO: add sort methods
    pub fn display_report<F: Fn(&Item) -> bool + Copy>(
        name: &str,
        report_list: &[&Item],
        style: ReportStyle,
        f: F,
    ) {
        eprintln!("{} | {} selected items", name, report_list.len());

        for item in report_list {
            print_item_styled(item, style, 0, f);
        }
    }
}

fn main() -> ExitCode {
    let home_path = getenv("HOME").unwrap();
    let itmn_file = getenv("ITMN_FILE").unwrap_or(format!("{}/.local/share/itmn", home_path));

    let options = cli::Options::parse();

    let pstring = match options.path {
        Some(cfg) => cfg,
        None => itmn_file,
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
            return ExitCode(1);
        }
    };

    let data: Vec<Item> = match ItemManager::import(&contents) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Failed to parse file: {}", e);
            return ExitCode(1);
        }
    };

    let mut manager = match ItemManager::new(data) {
        Ok(m) => m,
        Err(Error::RepeatedRefID(i)) => {
            eprintln!(
                "Repeated reference ID in file: {}; it'll have to be removed manually.",
                i
            );
            return ExitCode(1);
        }
        Err(Error::RepeatedInternalID(i)) => {
            eprintln!(
                "Repeated internal ID in file: {}; it'll have to be removed manually.",
                i
            );
            return ExitCode(1);
        }
    };

    match options.subcmd.unwrap_or(SubCmd::Next) {
        SubCmd::SelRefID(dt) => match core::misc::parse_range_str(&dt.range) {
            Ok(r) => {
                if r.is_empty() {
                    eprintln!("No selection was specified.");
                    return ExitCode(1);
                } else {
                    // find invalid IDs
                    if let Some(id) = core::misc::get_first_not_on_set(&r, &manager.ref_ids()) {
                        eprintln!("There is at least one invalid ID ({}) on the selection", id);
                        return ExitCode(1);
                    }

                    match dt.action.unwrap_or(SelectionAction::ListBrief) {
                        SelectionAction::Modify(m) => {
                            manager.mass_modify(&r, m);
                        }
                        SelectionAction::AddChild(dt) => {
                            if r.len() > 1 {
                                eprintln!("More than one item was selected. All of them will receive new identical children.");

                                if !core::misc::confirm_with_default(false) {
                                    return ExitCode(1);
                                }
                            }

                            for id in r {
                                manager
                                    .add_child_to_ref_id(
                                        id,
                                        dt.name.clone(),
                                        dt.context.clone(),
                                        match dt.note {
                                            Some(false) | None => State::Todo,
                                            Some(true) => State::Note,
                                        },
                                        Vec::new(),
                                    )
                                    .unwrap();
                            }
                        }
                        SelectionAction::Done => {
                            for id in r {
                                manager.interact_mut(id, |i| {
                                    if let State::Todo = i.state {
                                        i.state = State::Done;
                                    }
                                });
                            }
                        }
                        SelectionAction::ListTree => {
                            let sel_vec: Vec<&Item> =
                                r.iter().map(|id| manager.find(*id).unwrap()).collect();
                            report::display_report(
                                "Tree listing",
                                &sel_vec,
                                ReportStyle::Tree,
                                |_| true,
                            );
                        }
                        SelectionAction::ListBrief => {
                            let sel_vec: Vec<&Item> =
                                r.iter().map(|id| manager.find(*id).unwrap()).collect();
                            report::display_report(
                                "Tree listing",
                                &sel_vec,
                                ReportStyle::Brief,
                                |_| true,
                            );
                        }
                        SelectionAction::ListShallow => {
                            let sel_vec: Vec<&Item> =
                                r.iter().map(|id| manager.find(*id).unwrap()).collect();
                            report::display_report(
                                "Tree listing",
                                &sel_vec,
                                ReportStyle::Shallow,
                                |_| true,
                            );
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to parse range: {}", e);
                return ExitCode(1);
            }
        },
        SubCmd::Add(dt) => {
            manager.add_item_on_root(
                dt.name,
                dt.context,
                match dt.note {
                    Some(false) | None => State::Todo,
                    Some(true) => State::Note,
                },
                Vec::new(),
            );
        }
        SubCmd::List => {
            let items: Vec<&Item> = manager
                .get_surface_ref_ids()
                .iter()
                .map(|&i| manager.find(i).unwrap())
                .collect();
            report::display_report("All tasks (surface)", &items, ReportStyle::Tree, |i| {
                i.state != State::Done
            });
        }
        SubCmd::Next => {
            let items: Vec<&Item> = manager
                .get_surface_ref_ids()
                .iter()
                .map(|&i| manager.find(i).unwrap())
                .collect();
            report::display_report("Next", &items, ReportStyle::Brief, |i| {
                i.state == State::Todo
            });
        }
    }

    if let Err(e) = manager.save_if_modified(&path) {
        eprintln!("Failed to save changes to file: {}", e);
        ExitCode(1)
    } else {
        ExitCode(0)
    }
}
