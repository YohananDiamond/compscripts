//! Stores data structures related to the database's storage unit.

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
/// An item state describes whether said item is actionable (to do / done) or a note. More possible states might be
/// added on the future.
pub enum ItemState {
    /// The item is actionable, and is not yet marked as done.
    Todo,
    /// The item is actionable and is marked as done.
    Done,
    /// The item is not actionable, so it can't be marked as done.
    Note,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
/// The main data unit used to store information on this program's database.
pub struct Item {
    /// The public name of the item. It usually appears on most reports.
    pub name: String,
    /// The state of the item. See the documentation for ItemState for more information.
    pub state: ItemState,
    /// When an item is not marked as done (and is not a child of an item which is marked as done), it has a reference
    /// ID. This reference ID is automatically allocated to be as nearest possible to zero, making it simpler to mention
    /// items in increasingly larger item databases.
    pub ref_id: Option<u32>,
    /// An ID for internal representation of items. Each of these IDs are unique, even if the task is already marked as
    /// done. This is useful to help on referencing tasks which were already marked as done.
    pub internal_id: u32,
    #[serde(default)]
    /// Extra information for an item, with filetype = markdown by default.
    pub description: String,
    /// The children of this item, if any.
    ///
    /// According to the [`Vec`] documentation:
    ///
    /// > [...] if you construct a [`Vec`] with capacity 0 via [`Vec::new`], [`vec![]`][`vec!`], [`Vec::with_capacity(0)`], or by
    /// > calling [`shrink_to_fit`] on an empty [`Vec`], it will not allocate memory.
    ///
    /// So we don't need to make an `Option<Vec<Item>>` to avoid unecessary allocations.
    ///
    /// [`Vec`]: std::vec::Vec
    /// [`Vec::new`]: std::vec::Vec::new
    /// [`Vec::with_capacity(0)`]: std::vec::Vec::with_capacity
    /// [`shrink_to_fit`]: Vec::shrink_to_fit
    pub children: Vec<Item>,
    // TODO: creation_date: /* idk */,
    // TODO: defer_date: Option</* idk */>,
    // TODO: deprecate context (possibly)
    context: Option<String>,
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.internal_id.cmp(&other.internal_id)
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Item {
    /// Creates a new item.
    pub fn new(
        ref_id: Option<u32>,
        internal_id: u32,
        name: &str,
        context: &str, // we're gonna have to copy anyways... :v
        state: ItemState,
        description: String,
        children: Vec<Item>,
    ) -> Self {
        Self {
            ref_id,
            internal_id,
            name: Self::validate_name(name),
            context: Self::validate_context(context),
            state,
            description,
            children,
        }
    }

    /// Verifies if a context string would translate to a "no context" state.
    pub fn context_translates_to_null(string: &str) -> bool {
        matches!(string.to_lowercase().as_str(), ".void" | ".none" | "")
    }

    /// Processes a context string, returning whatever should be stored on the `context` field of the item.
    fn validate_context(context: &str) -> Option<String> {
        if Self::context_translates_to_null(&context) {
            None
        } else {
            Some(context.chars().filter(|&c| validate_char(c)).collect())
        }
    }

    /// Processes a name string, returning whatever should be stored on the `name` field of the item.
    fn validate_name(name: &str) -> String {
        name.chars().filter(|&c| validate_char(c)).collect()
    }

    #[inline]
    /// Returns an immutable reference to the item name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    /// Validates and sets the name of the item.
    pub fn set_name(&mut self, new_name: &str) {
        self.name = Self::validate_name(new_name);
    }

    #[inline]
    /// Returns an immutable reference to the context, if any.
    pub fn context(&self) -> Option<&str> {
        Some(self.context.as_ref()?.as_str())
    }

    #[inline]
    /// Validates and sets the context of the item.
    pub fn set_context(&mut self, new_context: &str) {
        self.context = Self::validate_context(new_context);
    }
}

/// A function that returns only valid characters for a name/context.
fn validate_char(c: char) -> bool {
    match c {
        '\n' | '\t' | '\r' => false,
        _ => true,
    }
}
