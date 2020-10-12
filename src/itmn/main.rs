#![feature(termination_trait_lib)]

use clap::Clap;
use std::path::Path;

mod cli;
mod item;
mod manager;

use cli::*;
use core::data::{Id, JsonSerializer, Manager};
use core::error::ExitCode;
use item::{Item, State};
use manager::{Error, ItemManager};
use report::ReportStyle;

use std::collections::HashSet;
use std::iter::FromIterator;

mod report {
    use super::{Item, State};

    #[derive(Clone, Copy)]
    pub enum ReportStyle {
        Shallow,
        Brief,
        Tree,
    }

    pub fn print_single_item<F>(item: &Item, indentation: usize, f: F)
    where
        F: Fn(&Item) -> bool + Copy,
    {
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

    pub fn print_item_styled<F>(item: &Item, style: ReportStyle, indentation: usize, f: F)
    where
        F: Fn(&Item) -> bool + Copy,
    {
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
    pub fn display_report<F>(name: &str, report_list: &[&Item], style: ReportStyle, f: F)
    where
        F: Fn(&Item) -> bool + Copy,
    {
        eprintln!("{} | {} selected items", name, report_list.len());

        for item in report_list {
            print_item_styled(item, style, 0, f);
        }
    }
}

fn main() -> ExitCode {
    let home_path = std::env::var("HOME").unwrap();
    let itmn_file =
        std::env::var("ITMN_FILE").unwrap_or(format!("{}/.local/share/itmn", home_path));

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
                }

                // abort if there's an invalid ID
                if let Some(id) = manager.get_first_invalid_ref_id(r.iter()) {
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
                            r.iter().map(|&id| manager.find(id).unwrap()).collect();

                        report::display_report("Tree listing", &sel_vec, ReportStyle::Tree, |_| {
                            true
                        });
                    }
                    SelectionAction::ListBrief => {
                        let sel_vec: Vec<&Item> =
                            r.iter().map(|&id| manager.find(id).unwrap()).collect();

                        report::display_report(
                            "Tree listing",
                            &sel_vec,
                            ReportStyle::Brief,
                            |_| true,
                        );
                    }
                    SelectionAction::ListShallow => {
                        let sel_vec: Vec<&Item> =
                            r.iter().map(|&id| manager.find(id).unwrap()).collect();

                        report::display_report(
                            "Tree listing",
                            &sel_vec,
                            ReportStyle::Shallow,
                            |_| true,
                        );
                    }
                    SelectionAction::Delete(args) => {
                        fn proceed(manager: &mut ItemManager, ids: Vec<Id>) {
                            let ids = HashSet::from_iter(ids);

                            fn do_the_thing(data: &mut Vec<Item>, ids: &HashSet<Id>) {
                                let mut i = 0;
                                while i < data.len() {
                                    if let Some(id) = data[i].ref_id {
                                        if ids.contains(&id) {
                                            // this will move the item at the end of the vector to the current position, which I don't
                                            // think is a problem, since we're not gonna increase i if the deletion happens
                                            data.swap_remove(i);
                                        } else {
                                            // do the same operation to the children, if there's any
                                            let children = &mut data[i].children;

                                            if children.len() > 0 {
                                                do_the_thing(children, ids);
                                            }

                                            // only increment here because, if the item is removed, everything is gonna be moved back
                                            i += 1;
                                        }
                                    }
                                }
                            }

                            do_the_thing(manager.data_mut(), &ids);
                            manager.after_interact_mut_hook();

                            // I don't think IDs need to be freed since the application will close soon
                        }

                        if !args.force.unwrap_or(false) {
                            let sel_vec: Vec<&Item> =
                                r.iter().map(|&id| manager.find(id).unwrap()).collect();

                            report::display_report(
                                "Items to be deleted",
                                &sel_vec,
                                ReportStyle::Tree,
                                |_| true,
                            );

                            eprintln!("Do you wish to delete these items?");
                            if core::misc::confirm_with_default(true) {
                                proceed(&mut manager, r.clone());
                            }
                        } else {
                            proceed(&mut manager, r.clone());
                        }
                    }
                    SelectionAction::Swap(args) => {
                        if r.len() != 2 {
                            eprintln!("The amount of args should be exactly two (it's {}).", r.len());
                            return ExitCode(1);
                        }

                        fn proceed(manager: &mut ItemManager, ids: Vec<Id>) {
                            unsafe {
                                let first: *mut Item = manager.find_mut(ids[0]).unwrap();
                                let second: *mut Item = manager.find_mut(ids[1]).unwrap();
                                std::ptr::swap(first, second);
                            }

                            manager.after_interact_mut_hook();
                        }

                        if !args.force.unwrap_or(false) {
                            let sel_vec: Vec<&Item> =
                                r.iter().map(|&id| manager.find(id).unwrap()).collect();

                            report::display_report(
                                "Items to be swapped",
                                &sel_vec,
                                ReportStyle::Shallow,
                                |_| true,
                            );

                            eprintln!("Do you wish to swap these items?");
                            eprintln!("Each item will keep their children.");
                            if core::misc::confirm_with_default(true) {
                                proceed(&mut manager, r.clone());
                            }
                        } else {
                            proceed(&mut manager, r.clone());
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
            report::display_report("All items (surface)", &items, ReportStyle::Tree, |i| {
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
