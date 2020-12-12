#![feature(termination_trait_lib)]

use clap::Clap;

use std::collections::HashSet;
use std::io;
use std::path::Path;

mod cli;
use cli::*;

mod item;
use item::{InternalId, Item, ItemState, RefId};

mod manager;
use manager::{Interactable, Searchable};
use manager::{ItemManager, ManagerError, ProgramResult};

mod report;
use report::{BasicReport, Report, ReportConfig, ReportDepth, ReportInfo};

use core::data::data_serialize;
use core::error::ExitCode;
use core::misc::confirm_with_default;
use core::tmp;

fn validate_parsed_string(string: &str) -> &str {
    for ch in string.chars() {
        if !matches!(ch, '\n' | ' ' | '\t' | '\r') {
            return string;
        }
    }

    "[]"
}

fn main() -> ExitCode {
    let itmn_file = std::env::var("ITMN_FILE")
        .unwrap_or_else(|_| format!("{}/.local/share/itmn", std::env::var("HOME").unwrap()));

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

    let data: Vec<Item> = match data_serialize::import(validate_parsed_string(&contents)) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to parse file: {}", e);
            return ExitCode(1);
        }
    };

    let mut manager = match ItemManager::new(data) {
        Ok(manager) => manager,
        Err(ManagerError::RepeatedRefID(RefId(id))) => {
            eprintln!(
                "Repeated reference ID in file: {}; it'll have to be removed manually.",
                id
            );
            return ExitCode(1);
        }
        Err(ManagerError::RepeatedInternalID(InternalId(id))) => {
            eprintln!(
                "Repeated internal ID in file: {}; it'll have to be removed manually.",
                id
            );
            return ExitCode(1);
        }
    };

    let code = manager.start_program_with_file(&path, |manager| {
        type UsedReport = BasicReport;
        const DEFAULT_SUBCOMMAND: SubCmd = SubCmd::Next;
        const SPACES_PER_INDENT: usize = 2;

        let report_cfg = ReportConfig {
            spaces_per_indent: SPACES_PER_INDENT,
        };

        let result = match subcmd.unwrap_or(DEFAULT_SUBCOMMAND) {
            SubCmd::SelRefID(args) => subcmd_selection::<UsedReport>(manager, args, &report_cfg),
            SubCmd::Add(args) => subcmd_add(manager, args),
            SubCmd::List => subcmd_list::<UsedReport>(manager, &report_cfg),
            SubCmd::Next => subcmd_next::<UsedReport>(manager, &report_cfg),
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
            Some(false) | None => ItemState::Todo,
            Some(true) => ItemState::Note,
        },
        String::new(), // description
        Vec::new(),    // children
    );

    Ok(ProgramResult {
        should_save: true,
        exit_status: 0,
    })
}

/// A function for the `list` subcommand.
///
/// Type argument `R` is the type of report that should be shown.
fn subcmd_list<R: Report>(
    manager: &ItemManager,
    report_cfg: &ReportConfig,
) -> Result<ProgramResult, String> {
    let items: Vec<&Item> = manager
        .surface_ref_ids()
        .iter()
        .map(|&i| manager.find(i).unwrap())
        .collect();

    R::report(
        "All items (surface)",
        &mut items.into_iter(),
        &ReportInfo {
            config: report_cfg,
            indent: 0,
            filter: Some(&|i: &Item| i.state != ItemState::Done),
            depth: ReportDepth::Tree,
        },
        &mut io::stderr(),
    )
    .unwrap();

    Ok(ProgramResult {
        should_save: false,
        exit_status: 0,
    })
}

/// A function for the `next` subcommand.
///
/// Type argument `R` is the type of report that should be shown.
fn subcmd_next<R: Report>(
    manager: &ItemManager,
    report_cfg: &ReportConfig,
) -> Result<ProgramResult, String> {
    let items: Vec<&Item> = manager
        .surface_ref_ids()
        .iter()
        .map(|&i| manager.find(i).unwrap())
        .collect();

    R::report(
        "Next",
        &mut items.into_iter(),
        &ReportInfo {
            config: report_cfg,
            indent: 0,
            filter: Some(&|i: &Item| i.state != ItemState::Done),
            depth: ReportDepth::Brief,
        },
        &mut io::stderr(),
    )
    .unwrap();

    Ok(ProgramResult {
        should_save: false,
        exit_status: 0,
    })
}

/// A function for the `sel-ref-id` subcommand.
///
/// Type argument `R` is the type of report that should be shown.
fn subcmd_selection<R: Report>(
    manager: &mut ItemManager,
    args: SelectionDetails,
    report_cfg: &ReportConfig,
) -> Result<ProgramResult, String> {
    type SelAct = SelectionAction;

    let range = match core::misc::parse_range_str(&args.range) {
        Ok(vec) => {
            // check if empty
            if vec.is_empty() {
                return Err("no selection was specified".into());
            }

            // abort if there's an invalid ID
            if let Some(RefId(missing)) = manager.first_invalid_ref_id(vec.iter()) {
                return Err(format!(
                    "there's at least one invalid ID (#{}) on the selection",
                    missing,
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

            R::report(
                "Items to be modified",
                &mut selected.into_iter(),
                &ReportInfo {
                    config: report_cfg,
                    indent: 0,
                    filter: None,
                    depth: ReportDepth::Shallow,
                },
                &mut io::stderr(),
            )
            .unwrap();

            eprintln!();

            let modifications = sargs.modifications_description();

            if modifications.is_empty() {
                eprintln!("No changes were specified");

                // Exit sucessfully though, I don't think this is necessarily a problem.
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
                for &id in &range {
                    manager
                        .add_child(
                            RefId(id),
                            &sargs.name,
                            sargs.context.as_ref().unwrap_or(&String::new()),
                            match sargs.note {
                                Some(false) | None => ItemState::Todo,
                                Some(true) => ItemState::Note,
                            },
                            sargs.description.clone().unwrap_or(String::new()),
                            Vec::new(), // children
                        )
                        .unwrap();
                }

                Ok(ProgramResult {
                    should_save: true,
                    exit_status: 0,
                })
            };

            if range.len() > 1 {
                eprintln!("More than one item was selected. All of them will receive new identical children copies.");

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
        SelAct::PrintDescription => {
            if range.len() != 1 {
                return Err("The selection should have exactly one item.".into());
            }

            manager
                .interact(RefId(range[0]), |i| {
                    // Check which char is the last one
                    match i.description.chars().rev().nth(0).unwrap_or('\n') {
                        '\n' => eprint!("{}", i.description),
                        _ => eprintln!("{}", i.description),
                    }

                    Ok(ProgramResult {
                        should_save: false,
                        exit_status: 0,
                    })
                })
                .unwrap()
        }
        SelAct::EditDescription => {
            if range.len() != 1 {
                return Err("The selection should have exactly one item.".into());
            }

            manager
                .interact_mut(RefId(range[0]), |i| {
                    match tmp::edit_text(&i.description, Some("md")) {
                        Ok((new_description, 0)) => {
                            i.description = new_description;

                            Ok(ProgramResult {
                                should_save: true,
                                exit_status: 0,
                            })
                        }
                        Ok((_, code)) => Err(format!("non-zero exit code: {}", code)),
                        Err(e) => Err(format!("failed to edit text: {}", e)),
                    }
                })
                .unwrap()
        }
        SelAct::Done => {
            for &id in &range {
                manager
                    .change_item_state(RefId(id), |previous| match previous {
                        ItemState::Todo => ItemState::Done,
                        other => other,
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

            R::report(
                "Tree listing",
                &mut selected.into_iter(),
                &ReportInfo {
                    config: report_cfg,
                    indent: 0,
                    filter: None,
                    depth: ReportDepth::Tree,
                },
                &mut io::stderr(),
            )
            .expect("Failed to show report");

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

            R::report(
                "Brief listing",
                &mut selected.into_iter(),
                &ReportInfo {
                    config: report_cfg,
                    indent: 0,
                    filter: None,
                    depth: ReportDepth::Brief,
                },
                &mut io::stderr(),
            )
            .expect("Failed to show report");

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

            R::report(
                "Shallow listing",
                &mut selected.into_iter(),
                &ReportInfo {
                    config: report_cfg,
                    indent: 0,
                    filter: None,
                    depth: ReportDepth::Shallow,
                },
                &mut io::stderr(),
            )
            .expect("Failed to show report");

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

                R::report(
                    "Items to be deleted",
                    &mut selection.into_iter(),
                    &ReportInfo {
                        config: report_cfg,
                        indent: 0,
                        filter: None,
                        depth: ReportDepth::Tree,
                    },
                    &mut io::stderr(),
                )
                .unwrap();

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

                R::report(
                    "Items to be swapped",
                    &mut selection.into_iter(),
                    &ReportInfo {
                        config: report_cfg,
                        indent: 0,
                        filter: None,
                        depth: ReportDepth::Brief,
                    },
                    &mut io::stderr(),
                )
                .unwrap();

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

            R::report(
                "Items to be moved",
                &mut items.into_iter(),
                &ReportInfo {
                    config: report_cfg,
                    indent: 0,
                    filter: None,
                    depth: ReportDepth::Shallow,
                },
                &mut io::stderr(),
            )
            .unwrap();

            let new_owner = match NewOwner::parse(&sargs.new_owner) {
                Ok(new) => new,
                Err(e) => return Err(format!("failed to parse new-owner argument: {}", e)),
            };

            let new_owner_internal_id = match new_owner {
                NewOwner::Root => eprintln!("New ownership: ROOT"),
                NewOwner::ByInternal(InternalId(id)) => {
                    if let Some(item) = manager.find(InternalId(id)) {
                        eprintln!("New owner: [I#{}] {}", id, item.name);
                        id
                    } else {
                        return Err(format!("could not find item with InternalId = {}", id));
                    }
                }
                NewOwner::ByRef(RefId(id)) => {
                    if let Some(item) = manager.find(RefId(id)) {
                        eprintln!("New owner: [R#{}] {}", id, item.name);
                        item.internal_id
                    } else {
                        return Err(format!("could not find item with RefId = {}", id));
                    }
                }
            };

            // Prevent the new owner from being in the selection
            for &id in &range {
                let item = manager.find(RefId(id)).unwrap();
                if item.internal_id == new_owner_internal_id {
                    return Err(format!(
                        r#"item "{name}" ({ref}I#{internal}) is on selection and is the new owner"#,
                        name = item.name,
                        r#ref = match item.ref_id {
                            Some(id) => &format!("R#{}, ", id),
                            None => "",
                        },
                        internal = format!("{}", item.internal_id)
                    ));
                }
            }

            eprintln!("Each item will keep its children.");

            if confirm_with_default(true) {
                let items: Vec<Item> = range
                    .iter()
                    .map(|&id| manager.try_remove(RefId(id)).unwrap()) // almost-safe (see TODOs below) unwrap due to range check
                    .collect();

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
