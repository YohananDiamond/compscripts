//! A module containing the command-line arguments, parsed by [`clap`].
//!
//! Most of this is left undocumented in the normal sense, since most options already have an `about` field.
//!
//! [`clap`]: clap

use clap::Clap;

use crate::item::{Item, ItemState};
use core::cowstr::CowStr;

#[derive(Debug, Clap)]
/// The entry point for the
pub struct Options {
    #[clap(
        short,
        long,
        about = "The path to the entries file (default: $ITMN_FILE => ~/.local/share/itmn)"
    )]
    pub path: Option<String>,
    #[clap(subcommand, about = "The command to be ran - defaults to [next]")]
    pub subcmd: Option<SubCmd>,
}

#[derive(Debug, Clap)]
pub enum SubCmd {
    // #[clap(subcommand, about = "Shows a report - defaults to [next]")]
    // TODO: Report(ReportSelection),
    #[clap(alias = "ls", about = "An alias to the [except-done] report")]
    List,
    #[clap(about = "An alias to the [next] report")]
    Next,
    #[clap(about = "Add an item")]
    Add(ItemAddDetails),
    #[clap(
        aliases = &["s", "sel", "sri"],
        about = "Select items by reference ID and do something with them",
    )]
    SelRefID(SelectionDetails),
    #[clap(
        aliases = &["flatlist", "fl"],
        about = "List all visible items, prepended by the ID",
    )]
    FlatList,
    // #[clap(aliases = &["sel-internal", "sii"], about = "Select items by internal ID and do something with them")]
    // TODO: SelInternalID(SelectionDetails),
    // TODO: Search,
    // TODO: RegexMatch,
}

#[derive(Debug, Clap)]
pub struct ItemAddDetails {
    #[clap(about = "The name of the item")]
    pub name: String,
    #[clap(short, long, about = "The context of the item")]
    pub context: Option<String>,
    #[clap(short, long, about = "If the item is a note")]
    pub note: Option<bool>,
    #[clap(short, long, about = "The description of the item")]
    pub description: Option<String>,
}

#[derive(Debug, Clap)]
pub struct SelectionDetails {
    #[clap(about = "The selection range")]
    pub range: String, // TODO: document range syntax
    #[clap(
        subcommand,
        about = "What to do with the selection, defaults to [list-tree]"
    )]
    pub action: Option<SelectionAction>,
}

#[derive(Debug, Clap)]
pub enum SelectionAction {
    #[clap(alias = "mod", about = "Modify the matches")]
    Modify(ItemBatchMod),
    #[clap(aliases = &["ac"], about = "Add a child to each one of the matches")]
    Add(ItemAddDetails),
    #[clap(about = "Mark the items on the selection as DONE, if their states are TODO")]
    Done,
    #[clap(alias = "tree", about = "List selection in a tree")]
    ListTree,
    #[clap(aliases = &["l", "ls", "list"], about = "List selection, showing only the first child of each, if any")]
    ListBrief,
    #[clap(about = "List selection without showing any children")]
    ListShallow,
    #[clap(aliases = &["del", "rm", "remove"], about = "Delete selected items")]
    Delete(ForceArgs),
    #[clap(about = "Swap two items")]
    Swap(ForceArgs),
    #[clap(alias = "chown", about = "Change ownership of the selected item(s)")]
    ChangeOwnership(ChownArgs),
    #[clap(aliases = &["ed", "edesc"], about = "Edit the description of an item")]
    EditDescription,
    #[clap(aliases = &["d", "desc"], about = "Print the description of an item")]
    PrintDescription,
}

#[derive(Debug, Clap, Clone)]
pub struct ItemBatchMod {
    #[clap(about = "The item's new name")]
    pub name: Option<String>,
    #[clap(
        short,
        long,
        about = "The item's new context; set to an empty string to unset"
    )]
    pub context: Option<String>,
    #[clap(short, long, about = "The item's new type")]
    pub note: Option<bool>,
}

impl ItemBatchMod {
    /// Describes what changes will be done to the item.
    pub fn modifications_description(&self) -> Vec<CowStr> {
        let mut vec = Vec::new();

        if let Some(name) = &self.name {
            vec.push(format!("Change name to {:?}", name).into());
        }

        if let Some(ctx) = &self.context {
            vec.push(if Item::context_translates_to_null(ctx) {
                "Remove context".into()
            } else {
                format!("Change context to {:?}", ctx).into()
            });
        }

        if let Some(note) = self.note {
            if note {
                vec.push("Transform into a note".into());
            } else {
                vec.push("Transform into an actionable item (task)".into());
            }
        }

        vec
    }

    /// Apply modifications to an item.
    pub fn mod_item(self, item: &mut Item) {
        if let Some(name) = self.name {
            item.name = name;
        }

        if let Some(context) = self.context {
            item.set_context(&context);
        }

        if let Some(note) = self.note {
            if note {
                item.state = ItemState::Note;
            } else {
                // only change to active/pending if item is actually a note
                if let ItemState::Note = item.state {
                    item.state = ItemState::Todo;
                }
            }
        }
    }
}

#[derive(Debug, Clap)]
/// A simple argument to help with common --force commands.
pub struct ForceArgs {
    #[clap(short, long, about = "Skip warning/confirmation messages (unsafe)")]
    pub force: Option<bool>,
}

impl Into<bool> for ForceArgs {
    fn into(self) -> bool {
        self.force.unwrap_or(false)
    }
}

#[derive(Debug, Clap)]
pub struct ChownArgs {
    #[clap(
        about = "the new owner of the task. Should be .ROOT, a reference ID, or an internal ID - prefixed by i"
    )]
    pub new_owner: String,
}
