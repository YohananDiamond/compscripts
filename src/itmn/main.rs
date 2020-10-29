#![feature(termination_trait_lib)]

// TODO: remove ref IDs from children when their parents are marked as done

use clap::Clap;

use std::collections::HashSet;
use std::path::Path;

mod cli;
use cli::*;

mod item;
use item::{Item, State};

mod manager;
use manager::{Error, InternalId, ItemManager, ProgramResult, RefId};
use manager::{Interactable, Searchable};

mod report;
use report::{ReportManager, ReportStyle};

use core::data::data_serialize;
use core::error::ExitCode;
use core::misc::confirm_with_default;

fn fallback_string_if_needed<'a>(string: &'a str) -> &'a str {
    for ch in string.chars() {
        if !matches!(ch, '\n' | ' ' | '\t' | '\r') {
            return string;
        }
    }

    "[]"
}

fn main() -> ExitCode {
    let home_path = std::env::var("HOME").unwrap();
    let itmn_file =
        std::env::var("ITMN_FILE").unwrap_or(format!("{}/.local/share/itmn", home_path));

    let options = cli::Options::parse();
    let subcmd = options.subcmd;
    let path_string = options.path.unwrap_or(itmn_file);
    let path = Path::new(&path_string);

    let contents = match core::io::touch_read(&path) {
        Ok(string) => string,
        Err(e) => {
            eprintln!("Failed to load file: {}", e);
            return ExitCode(1);
        }
    };

    let data: Vec<Item> = match data_serialize::import(fallback_string_if_needed(&contents)) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to parse file: {}", e);
            return ExitCode(1);
        }
    };

    let mut manager = match ItemManager::new(data) {
        Ok(manager) => manager,
        Err(Error::RepeatedRefID(id)) => {
            eprintln!(
                "Repeated reference ID in file: {}; it'll have to be removed manually.",
                id.0
            );
            return ExitCode(1);
        }
        Err(Error::RepeatedInternalID(id)) => {
            eprintln!(
                "Repeated internal ID in file: {}; it'll have to be removed manually.",
                id.0
            );
            return ExitCode(1);
        }
    };

    let code = manager.start_program_with_file(&path, |manager| {
        const DEFAULT_SUBCOMMAND: SubCmd = SubCmd::Next;
        const SPACES_PER_INDENT: usize = 2;

        let report_manager = ReportManager {
            spaces_per_indent: SPACES_PER_INDENT,
        };

        let result = match subcmd.unwrap_or(DEFAULT_SUBCOMMAND) {
            SubCmd::SelRefID(args) => subcmd_selection(manager, &report_manager, args),
            SubCmd::Add(args) => subcmd_add(manager, args),
            SubCmd::List => subcmd_list(manager, &report_manager),
            SubCmd::Next => subcmd_next(manager, &report_manager),
        };

        match result {
            Ok(pr) => pr,
            Err(e) => {
                eprintln!("Error: {}", e);
                ProgramResult {
                    should_save: false,
                    exit_status: 1,
                }
            }
        }
    });

    ExitCode(code)
}

fn subcmd_add(manager: &mut ItemManager, args: ItemAddDetails) -> Result<ProgramResult, String> {
    manager.add_item_on_root(
        &args.name,
        &args.context.unwrap_or(String::new()),
        match args.note {
            Some(false) | None => State::Todo,
            Some(true) => State::Note,
        },
        Vec::new(),
    );

    Ok(ProgramResult {
        should_save: true,
        exit_status: 0,
    })
}

fn subcmd_list(
    manager: &ItemManager,
    report_manager: &ReportManager,
) -> Result<ProgramResult, String> {
    let items: Vec<&Item> = manager
        .surface_ref_ids()
        .iter()
        .map(|&i| manager.find(i).unwrap())
        .collect();

    report_manager.display_report("All items (surface)", &items, ReportStyle::Tree, |i| {
        i.state != State::Done
    });

    Ok(ProgramResult {
        should_save: false,
        exit_status: 0,
    })
}

fn subcmd_next(
    manager: &ItemManager,
    report_manager: &ReportManager,
) -> Result<ProgramResult, String> {
    let items: Vec<&Item> = manager
        .surface_ref_ids()
        .iter()
        .map(|&i| manager.find(i).unwrap())
        .collect();

    report_manager.display_report("Next", &items, ReportStyle::Brief, |i| {
        i.state != State::Done
    });

    Ok(ProgramResult {
        should_save: false,
        exit_status: 0,
    })
}

fn subcmd_selection(
    manager: &mut ItemManager,
    report_manager: &ReportManager,
    args: SelectionDetails,
) -> Result<ProgramResult, String> {
    type SelAct = SelectionAction;

    let range = match core::misc::parse_range_str(&args.range) {
        Ok(vec) => {
            // check if empty
            if vec.is_empty() {
                return Err("no selection was specified".into());
            }

            // abort if there's an invalid ID
            if let Some(missing) = manager.first_invalid_ref_id(vec.iter()) {
                return Err(format!(
                    "there's at least one invalid ID (#{}) on the selection",
                    missing.0,
                ));
            }

            vec
        }
        Err(e) => {
            return Err(format!("failed to parse range: {}", e));
        }
    };

    match args.action.unwrap_or(SelAct::ListBrief) {
        SelAct::Modify(sargs) => {
            let proceed = |manager: &mut ItemManager| {
                for &id in &range {
                    // Now that I think of it, cloning the same thing over and over might be too expensive. But I don't know if it's better to try something like Rc to improve this.
                    manager.interact_mut(RefId(id), |item| sargs.clone().mod_item(item));
                }

                Ok(ProgramResult {
                    should_save: true,
                    exit_status: 0,
                })
            };

            let selected: Vec<&Item> = range
                .iter()
                .map(|&id| manager.find(RefId(id)).unwrap())
                .collect();

            report_manager.display_report(
                "Items to be modified",
                &selected,
                ReportStyle::Shallow,
                |_| true,
            );

            eprintln!();

            let modifications = sargs.modifications_description();

            if modifications.is_empty() {
                eprintln!("No changes were specified");

                // exit sucessfully though, I don't think this is necessarily a problem.
                Ok(ProgramResult {
                    should_save: false,
                    exit_status: 0,
                })
            } else {
                eprintln!("Changes to be made:");
                for modification in sargs.modifications_description() {
                    eprintln!(" * {}", modification);
                }

                if confirm_with_default(true) {
                    proceed(manager)
                } else {
                    Ok(ProgramResult {
                        should_save: false,
                        exit_status: 1,
                    })
                }
            }
        }
        SelAct::Add(sargs) => {
            let mut proceed = || {
                for id in &range {
                    manager
                        .add_child(
                            RefId(*id),
                            &sargs.name,
                            match sargs.context.as_ref() {
                                Some(sref) => sref,
                                None => "",
                            },
                            match sargs.note {
                                Some(false) | None => State::Todo,
                                Some(true) => State::Note,
                            },
                            Vec::new(),
                        )
                        .unwrap();
                }

                Ok(ProgramResult {
                    should_save: true,
                    exit_status: 0,
                })
            };

            if range.len() > 1 {
                eprintln!("More than one item was selected. All of them will receive new identical children.");

                if !confirm_with_default(false) {
                    proceed()
                } else {
                    Ok(ProgramResult {
                        should_save: false,
                        exit_status: 1,
                    })
                }
            } else {
                proceed()
            }
        }
        SelAct::Done => {
            for id in &range {
                manager
                    .interact_mut(RefId(*id), |i| {
                        if let State::Todo = i.state {
                            i.state = State::Done;
                        }
                    })
                    .unwrap(); // safe because we already made sure all IDs in the range exist.
            }

            Ok(ProgramResult {
                should_save: true,
                exit_status: 0,
            })
        }
        SelAct::ListTree => {
            let selected: Vec<&Item> = range
                .iter()
                .map(|&id| manager.find(RefId(id)).unwrap())
                .collect();

            report_manager.display_report("Tree listing", &selected, ReportStyle::Tree, |_| true);

            Ok(ProgramResult {
                should_save: false,
                exit_status: 0,
            })
        }
        SelAct::ListBrief => {
            let selected: Vec<&Item> = range
                .iter()
                .map(|&id| manager.find(RefId(id)).unwrap())
                .collect();

            report_manager.display_report("Brief listing", &selected, ReportStyle::Brief, |_| true);

            Ok(ProgramResult {
                should_save: false,
                exit_status: 0,
            })
        }
        SelAct::ListShallow => {
            let selected: Vec<&Item> = range
                .iter()
                .map(|&id| manager.find(RefId(id)).unwrap())
                .collect();

            report_manager.display_report(
                "Shallow listing",
                &selected,
                ReportStyle::Shallow,
                |_| true,
            );

            Ok(ProgramResult {
                should_save: false,
                exit_status: 0,
            })
        }
        SelAct::Delete(sargs) => {
            /// Iterates recursively of a vector of items and their children, removing any items that are on the
            /// selection. IDs on the selection that aren't found will be ignored. This is probably not a problem
            /// because we already made sure the selection passed here has only valid IDs, so any missing IDs are from
            /// children of items that were already deleted on this run.
            fn thing(data: &mut Vec<Item>, selection: &HashSet<RefId>) {
                data.retain(|item| {
                    if let Some(id) = item.ref_id {
                        if selection.contains(&RefId(id)) {
                            return false;
                        }
                    }

                    true
                });

                for item in data.iter_mut() {
                    thing(&mut item.children, selection);
                }
            }

            let proceed = |manager: &mut ItemManager| {
                thing(
                    &mut manager.data,
                    &range.iter().map(|&id| RefId(id)).collect(),
                );

                // I don't think IDs need to be freed since the application closes soon after this, but that might be a
                // thing to worry on the future.

                Ok(ProgramResult {
                    should_save: true,
                    exit_status: 0,
                })
            };

            if !sargs.force.unwrap_or(false) {
                let selection: Vec<&Item> = range
                    .iter()
                    .map(|&id| manager.find(RefId(id)).unwrap())
                    .collect();

                report_manager.display_report(
                    "Items to be deleted",
                    &selection,
                    ReportStyle::Tree,
                    |_| true,
                );

                if confirm_with_default(true) {
                    proceed(manager)
                } else {
                    Ok(ProgramResult {
                        should_save: false,
                        exit_status: 1,
                    })
                }
            } else {
                proceed(manager)
            }
        }
        SelAct::Swap(sargs) => {
            if range.len() != 2 {
                return Err(format!(
                    "the amount of arguments should be exactly two (instead of {})",
                    range.len()
                ));
            }

            let proceed =
                |manager: &mut ItemManager| match manager.swap(RefId(range[0]), RefId(range[1])) {
                    Ok(()) => Ok(ProgramResult {
                        should_save: true,
                        exit_status: 0,
                    }),
                    Err(e) => Err(format!("item swap failed: {}", e)),
                };

            if !sargs.force.unwrap_or(false) {
                let selection: Vec<&Item> = range
                    .iter()
                    .map(|&id| manager.find(RefId(id)).unwrap())
                    .collect();

                report_manager.display_report(
                    "Items to be swapped",
                    &selection,
                    ReportStyle::Brief,
                    |_| true,
                );

                eprintln!("Each item will keep their children.");
                if confirm_with_default(true) {
                    proceed(manager)
                } else {
                    Ok(ProgramResult {
                        should_save: false,
                        exit_status: 1,
                    })
                }
            } else {
                proceed(manager)
            }
        }
        SelAct::ChangeOwnership(sargs) => {
            enum NewOwner {
                Root,
                ByInternal(InternalId),
                ByRef(RefId),
            }

            impl NewOwner {
                pub fn parse(arg: &str) -> Result<Self, String> {
                    if arg == ".ROOT" {
                        // Parse ROOT
                        Ok(Self::Root)
                    } else if let Some('i') = arg.chars().nth(0) {
                        // Parse Internal ID
                        if let Ok(num) = (&arg[1..]).parse::<u32>() {
                            Ok(Self::ByInternal(InternalId(num)))
                        } else {
                            Err(format!(
                                "invalid number after 'i' character: {:?}",
                                &arg[1..]
                            ))
                        }
                    } else if let Ok(num) = arg.parse::<u32>() {
                        Ok(Self::ByRef(RefId(num)))
                    } else {
                        Err(format!("invalid expression: {:?}", arg))
                    }
                }
            }

            let items: Vec<&Item> = range
                .iter()
                .map(|&id| manager.find(RefId(id)).unwrap())
                .collect();

            report_manager.display_report(
                "Items to be moved",
                &items,
                ReportStyle::Shallow,
                |_| true,
            );

            let new_owner = match NewOwner::parse(&sargs.new_owner) {
                Ok(new) => new,
                Err(e) => return Err(format!("failed to parse new-owner argument: {}", e)),
            };

            match new_owner {
                NewOwner::Root => eprintln!("New ownership: ROOT"),
                NewOwner::ByInternal(id) => {
                    if let Some(item) = manager.find(id) {
                        eprintln!("New ownership: [I#{}] {}", id.0, item.name);
                    } else {
                        return Err(format!("could not find item with InternalId = {}", id.0));
                    }
                }
                NewOwner::ByRef(id) => {
                    if let Some(item) = manager.find(id) {
                        eprintln!("New ownership: [R#{}] {}", id.0, item.name);
                    } else {
                        return Err(format!("could not find item with RefId = {}", id.0));
                    }
                }
            }

            eprintln!("Each item will keep its children.");

            if confirm_with_default(true) {
                let items: Vec<Item> = range
                    .iter()
                    .map(|&id| manager.try_remove(RefId(id)).unwrap()) // almost-safe (see TODOs below) unwrap due to range check
                    .collect();

                // TODO: prevent the new owner from being in the selection
                // TODO: prevent a selected item from being a child of another selected item (for now)

                match new_owner {
                    NewOwner::Root => manager.data.extend(items),
                    NewOwner::ByRef(id) => {
                        let owner = manager.find_mut(id).unwrap();
                        owner.children.extend(items);
                    }
                    NewOwner::ByInternal(id) => {
                        let owner = manager.find_mut(id).unwrap();
                        owner.children.extend(items);
                    }
                }

                Ok(ProgramResult {
                    should_save: true,
                    exit_status: 0,
                })
            } else {
                Ok(ProgramResult {
                    should_save: false,
                    exit_status: 1,
                })
            }
        }
    }
}
