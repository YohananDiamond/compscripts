#![feature(iter_intersperse)]

use clap::Parser;

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
use report::{FlatReport, Report, ReportConfig, ReportDepth, ReportInfo};

use utils::data::data_serialize;
use utils::error::ExitCode;
use utils::misc::confirm_with_default;
use utils::tmp;

fn main() -> ExitCode {
    let itmn_file = std::env::var("ITMN_FILE")
        .unwrap_or_else(|_| format!("{}/.local/share/itmn", std::env::var("HOME").unwrap()));

    let options = cli::Options::parse();
    let subcmd = options.subcmd;
    let path_string = options.path.unwrap_or(itmn_file);
    let path = Path::new(&path_string);

    const LOCK_NAME: &str = "itmn";
    let _lock = match utils::tmp::make_folder_lock(LOCK_NAME) {
        Ok(lock) => lock,
        Err(why) => {
            eprintln!("Failed to create lock `{}`: {}", LOCK_NAME, why);
            return ExitCode::new(1);
        }
    };

    let contents = match utils::io::touch_read(&path) {
        Ok(string) => string,
        Err(why) => {
            eprintln!("Failed to load file: {}", why);
            return ExitCode::new(1);
        }
    };

    let data: Vec<Item> = match data_serialize::import(validate_parsed_string(&contents)) {
        Ok(data) => data,
        Err(why) => {
            eprintln!("Failed to parse file: {}", why);
            return ExitCode::new(1);
        }
    };

    let mut manager = match ItemManager::new(data) {
        Ok(manager) => manager,
        Err(ManagerError::RepeatedRefID(RefId(id))) => {
            eprintln!(
                "Repeated reference ID in file: {}; it'll have to be removed manually.",
                id
            );
            return ExitCode::new(1);
        }
        Err(ManagerError::RepeatedInternalID(InternalId(id))) => {
            eprintln!(
                "Repeated internal ID in file: {}; it'll have to be removed manually.",
                id
            );
            return ExitCode::new(1);
        }
    };

    let code = manager.start_program_with_file(&path, |manager| {
        type UsedReport = report::BasicReport;
        const DEFAULT_SUBCOMMAND: SubCmd = SubCmd::List;
        const DEFAULT_SPACES_PER_INDENT: usize = 2;

        let report_cfg = ReportConfig {
            spaces_per_indent: DEFAULT_SPACES_PER_INDENT,
        };

        let result = match subcmd.unwrap_or(DEFAULT_SUBCOMMAND) {
            SubCmd::SelRefID(args) => subcmd_selection::<UsedReport>(manager, args, &report_cfg),
            SubCmd::Add(args) => subcmd_add(manager, args),
            SubCmd::List => subcmd_list::<UsedReport>(manager, &report_cfg),
            SubCmd::Next => subcmd_next::<UsedReport>(manager, &report_cfg),
            SubCmd::FlatList => subcmd_flatlist(manager, &report_cfg),
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

    ExitCode::new(code)
}

fn subcmd_add(
    manager: &mut ItemManager,
    ItemAddDetails {
        name,
        context,
        note,
        description,
    }: ItemAddDetails,
) -> Result<ProgramResult, String> {
    let RefId(ref_id) = manager.add_item_on_root(
        &name,
        &context.unwrap_or(String::new()),
        match note {
            Some(false) | None => ItemState::Todo,
            Some(true) => ItemState::Note,
        },
        description.unwrap_or_else(String::new), // description
        Vec::new(),                              // children
    );

    eprintln!("Item Added! | RefID: {}", ref_id);

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
        &mut io::stdout(),
    )
    .unwrap();

    Ok(ProgramResult {
        should_save: false,
        exit_status: 0,
    })
}

/// A function for the `flat-list` subcommand.
fn subcmd_flatlist(
    manager: &ItemManager,
    report_cfg: &ReportConfig,
) -> Result<ProgramResult, String> {
    let items: Vec<&Item> = manager
        .surface_ref_ids()
        .iter()
        .map(|&i| manager.find(i).unwrap())
        .collect();

    FlatReport::report(
        "All items (flat report)",
        &mut items.into_iter(),
        &ReportInfo {
            config: report_cfg,
            indent: 0,
            filter: Some(&|i: &Item| i.state != ItemState::Done),
            depth: ReportDepth::Tree,
        },
        &mut io::stdout(),
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
        &mut io::stdout(),
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

    let range = match utils::misc::parse_range_str(&args.range) {
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

    match args.action.unwrap_or(SelAct::ListTree) {
        SelAct::Modify(sargs) => {
            let proceed = |manager: &mut ItemManager| {
                for &id in &range {
                    manager.interact_mut(RefId(id), |item| sargs.mod_item_by_ref(item));
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
                &mut io::stdout(),
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
                eprintln!("Adding items:");

                for &id in &range {
                    let RefId(ref_id) = manager
                        .add_child(
                            RefId(id),
                            &sargs.name,
                            sargs.context.as_ref().map_or("", String::as_ref),
                            match sargs.note {
                                Some(false) | None => ItemState::Todo,
                                Some(true) => ItemState::Note,
                            },
                            sargs.description.clone().unwrap_or_else(String::new),
                            Vec::new(), // children
                        )
                        .unwrap();

                    eprintln!("* RefID: {}", ref_id);
                }

                Ok(ProgramResult {
                    should_save: true,
                    exit_status: 0,
                })
            };

            if range.len() > 1 {
                eprintln!("More than one item was selected. All of them will receive new identical children copies.");

                if confirm_with_default(false) {
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
        SelAct::EditName => {
            let name_lines: Vec<(u32, String)> = range
                .iter()
                .map(|&id| {
                    (
                        id,
                        manager
                            .interact_mut(RefId(id), |item| item.name.clone()) // holy shit I don't wanna clone
                            .unwrap(),
                    )
                })
                .collect();

            let names_string = name_lines.iter().map(|(_, s)| s.as_str()).intersperse("\n").collect::<String>();

            let edited_string = match tmp::edit_text(&names_string, Some("txt")) {
                Ok((new, 0)) => new,
                Ok((_, code)) => return Err(format!("non-zero exit code: {}", code)),
                Err(e) => return Err(format!("failed to edit text: {}", e)),
            };

            let edited_lines = edited_string
                .split('\n')
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>();

            if name_lines.len() != edited_lines.len() {
                return Err(format!("Incompatible amount of lines: {} (selection size) and {} (amount after editing)", name_lines.len(), edited_lines.len()));
            }

            for (&id, new_name) in name_lines.iter().map(|(id, _)| id).zip(edited_lines.iter()) {
                manager
                    .interact_mut(RefId(id), |i| {
                        i.set_name(new_name);
                    })
                    .unwrap();
            }

            Ok(ProgramResult {
                should_save: true,
                exit_status: 0,
            })
        }
        SelAct::EditDescription => {
            if range.len() != 1 {
                return Err("The selection should have exactly one item.".into());
            }

            manager
                .interact_mut(RefId(range[0]), |i| {
                    match tmp::edit_text(&i.description, Some("txt")) {
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
            let selection: Vec<&Item> = range
                .iter()
                .map(|&id| manager.find(RefId(id)).unwrap())
                .collect();

            R::report(
                "Items to be marked as done",
                &mut selection.into_iter(),
                &ReportInfo {
                    config: report_cfg,
                    indent: 0,
                    filter: None,
                    depth: ReportDepth::Tree,
                },
                &mut io::stdout(),
            )
            .unwrap();

            if confirm_with_default(true) {
                for &id in &range {
                    manager
                        .change_item_state(RefId(id), |previous| match previous {
                            // TODO: rename to map_state
                            ItemState::Todo => ItemState::Done,
                            other => other,
                        })
                        .unwrap(); // safe because we already made sure all IDs in the range exist.
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
                &mut io::stdout(),
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
                &mut io::stdout(),
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
                &mut io::stdout(),
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
                    &mut io::stdout(),
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
                    &mut io::stdout(),
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
                &mut io::stdout(),
            )
            .unwrap();

            let new_owner = match NewOwner::parse(&sargs.new_owner) {
                Ok(new) => new,
                Err(e) => return Err(format!("failed to parse new-owner argument: {}", e)),
            };

            eprintln!();

            let new_owner_internal_id = match new_owner {
                NewOwner::Root => {
                    eprintln!("New owner: ROOT");
                    None
                }
                NewOwner::ByInternal(InternalId(id)) => {
                    if let Some(item) = manager.find(InternalId(id)) {
                        eprintln!("New owner: {:?} (I#{})", item.name, id);
                        Some(id)
                    } else {
                        return Err(format!("could not find item with InternalId = {}", id));
                    }
                }
                NewOwner::ByRef(RefId(id)) => {
                    if let Some(item) = manager.find(RefId(id)) {
                        eprintln!("New owner: {:?} (R#{})", item.name, id);
                        Some(item.internal_id)
                    } else {
                        return Err(format!("could not find item with RefId = {}", id));
                    }
                }
            };

            {
                let items: Vec<_> = range
                    .iter()
                    .map(|&id| manager.find(RefId(id)).unwrap())
                    .collect();

                // Prevent the new owner from being in the selection
                for item in &items {
                    if Some(item.internal_id) == new_owner_internal_id {
                        return Err(format!(
                            r#"item "{name}" ({ref}I#{internal}) is on selection and is the new owner"#,
                            name = item.name,
                            r#ref = match item.ref_id {
                                Some(id) => format!("R#{}, ", id),
                                None => String::new(),
                            },
                            internal = format!("{}", item.internal_id)
                        ));
                    }
                }

                // Prevent a selected item from being a child of another selected item (for now)
                for (i, item) in items.iter().enumerate() {
                    for (i2, item2) in items.iter().enumerate() {
                        if i != i2 && item.has_child(item2) {
                            return Err(format!(
                                r#"parent-child conflict:
let item A = {c_name:?} ({c_ref}I#{c_internal}), and
    item B = {p_name:?} ({p_ref}I#{p_internal}).
A is a child of B, but both A and B are on the selection."#,
                                // Child
                                c_name = item2.name,
                                c_ref = match item2.ref_id {
                                    Some(id) => format!("R#{}, ", id),
                                    None => String::new(),
                                },
                                c_internal = format!("{}", item2.internal_id),
                                // Parent
                                p_name = item.name,
                                p_ref = match item.ref_id {
                                    Some(id) => format!("R#{}, ", id),
                                    None => String::new(),
                                },
                                p_internal = format!("{}", item.internal_id),
                            ));
                        }
                    }
                }
            }

            eprintln!("Each item will keep its children.");

            if confirm_with_default(true) {
                let items: Vec<Item> = range
                    .iter()
                    .map(|&id| manager.try_remove(RefId(id)).unwrap()) // safe unwrap due to range check
                    .collect();

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

fn validate_parsed_string(string: &str) -> &str {
    for ch in string.chars() {
        if !matches!(ch, '\n' | ' ' | '\t' | '\r') {
            return string;
        }
    }

    "[]"
}
