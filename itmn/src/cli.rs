//! A module containing the command-line arguments, parsed by [`clap`].
//!
//! Most of this is left undocumented in the normal sense, since most options already have an `about` field.
//!
//! [`clap`]: clap

use clap::{Parser, Subcommand};

use std::borrow::Cow;

use crate::item::{Item, ItemState};

#[derive(Debug, Parser, Clone)]
pub struct Options {
    #[arg(
        short,
        long,
        help = "The path to the entries file (default: $ITMN_FILE => ~/.local/share/itmn)"
    )]
    pub path: Option<String>,

    #[command(subcommand)]
    pub subcmd: Option<SubCmd>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SubCmd {
    // #[command(about = "Shows a report - defaults to [next]")]
    // TODO: Report(ReportSelection),
    #[command(alias = "ls", about = "An alias to the [except-done] report")]
    List,
    #[command(about = "An alias to the [next] report")]
    Next,
    #[command(about = "Add an item")]
    Add(ItemAddDetails),
    #[command(
        aliases = &["s", "sel", "sri"],
        about = "Select items by reference ID and do something with them",
    )]
    SelRefID(SelectionDetails),
    #[command(
        aliases = &["flatlist", "fl"],
        about = "List all visible items, prepended by the ID",
    )]
    FlatList,
    // #[command(aliases = &["sel-internal", "sii"], about = "Select items by internal ID and do something with them")]
    // TODO: SelInternalID(SelectionDetails),
    // TODO: Search,
    // TODO: RegexMatch,
}

#[derive(Debug, Parser, Clone)]
pub struct ItemAddDetails {
    #[arg(help = "The name of the item")]
    pub name: String,
    #[arg(short, long, help = "The context of the item")]
    pub context: Option<String>,
    #[arg(short, long, help = "If the item is a note")]
    pub note: Option<bool>,
    #[arg(short, long, help = "The description of the item")]
    pub description: Option<String>,
}

#[derive(Debug, Parser, Clone)]
pub struct SelectionDetails {
    #[arg(help = "The selection range")]
    pub range: String, // TODO: document range syntax
    #[command(subcommand)]
    pub action: Option<SelectionAction>,
}

#[derive(Debug, Subcommand, Clone)]
pub enum SelectionAction {
    #[command(alias = "mod", about = "Modify the matches")]
    Modify(ItemBatchMod),
    #[command(aliases = &["et", "en", "edit-title"], about = "Edit the matches' names (one per line)")]
    EditName,
    #[command(aliases = &["ac"], about = "Add a child to each one of the matches")]
    Add(ItemAddDetails),
    #[command(about = "Mark the items on the selection as DONE, if their states are TODO")]
    Done,
    #[command(alias = "tree", about = "List selection in a tree")]
    ListTree,
    #[command(aliases = &["l", "ls", "list"], about = "List selection, showing only the first child of each, if any")]
    ListBrief,
    #[command(about = "List selection without showing any children")]
    ListShallow,
    #[command(aliases = &["del", "rm", "remove"], about = "Delete selected items")]
    Delete(ForceArgs),
    #[command(about = "Swap two items")]
    Swap(ForceArgs),
    #[command(alias = "chown", about = "Change ownership of the selected item(s)")]
    ChangeOwnership(ChownArgs),
    #[command(aliases = &["ed", "edesc"], about = "Edit the description of an item")]
    EditDescription,
    #[command(aliases = &["d", "desc"], about = "Print the description of an item")]
    PrintDescription,
}

#[derive(Debug, Parser, Clone)]
pub struct ItemBatchMod {
    #[arg(help = "The item's new name")]
    pub name: Option<String>,
    #[arg(
        short,
        long,
        help = "The item's new context; set to an empty string to unset"
    )]
    pub context: Option<String>,
    #[arg(short, long, help = "The item's new type")]
    pub note: Option<bool>,
}

impl ItemBatchMod {
    /// Describes what changes will be done to the item.
    pub fn modifications_description(&self) -> Vec<Cow<'static, str>> {
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

    /// Apply modifications to an item, without consuming self.
    ///
    /// Might clone some of the contents of self, but not necessarily all.
    pub fn mod_item_by_ref(&self, item: &mut Item) {
        if let Some(ref name) = self.name {
            item.name = name.clone();
        }

        if let Some(ref context) = self.context {
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

    /// Apply modifications to an item, consuming self.
    #[allow(unused)]
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

#[derive(Debug, Parser, Clone)]
/// A simple argument to help with common --force commands.
pub struct ForceArgs {
    #[arg(short, long, help = "Skip warning/confirmation messages (unsafe)")]
    pub force: Option<bool>,
}

impl Into<bool> for ForceArgs {
    fn into(self) -> bool {
        self.force.unwrap_or(false)
    }
}

#[derive(Debug, Parser, Clone)]
pub struct ChownArgs {
    #[arg(
        help = "the new owner of the task. Should be .ROOT, a reference ID, or an internal ID - prefixed by i"
    )]
    pub new_owner: String,
}
