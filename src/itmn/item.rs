use core::data::Searchable;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum State {
    Todo,
    Done,
    Note,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Item {
    /// None means that this task doesn't have a reference ID. This only happens to completed tasks.
    /// TODO: finish doc for this
    pub ref_id: Option<u32>,
    /// The ID for internal representation, and also for managing hidden tasks (such as the "hidden" ones).
    pub internal_id: u32,
    /// The name of the item.
    pub name: String,
    /// The context of the item, GTD-style.
    context: Option<String>,
    /// The state of the item (note, todo, done etc.)
    pub state: State,
    /// The description of the item.
    /// Defaults to an empty description if not present already.
    #[serde(default)]
    pub description: String,
    /// The children of this item, if any.
    pub children: Vec<Item>,
    // TODO: creation_date: /* idk */,
    // TODO: defer_date: Option</* idk */>,
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

impl Searchable for Item {
    fn ref_id(&self) -> Option<u32> {
        self.ref_id
    }
}

impl Item {
    pub fn new(
        ref_id: Option<u32>,
        internal_id: u32,
        name: &str,
        context: &str, // we're gonna have to copy anyways... :v
        state: State,
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

    pub fn context_translates_to_null(string: &str) -> bool {
        matches!(string.to_lowercase().as_str(), ".void" | ".none" | "")
    }

    fn validate_context(context: &str) -> Option<String> {
        if Self::context_translates_to_null(&context) {
            None
        } else {
            Some(context.chars().filter(|&c| validate_char(c)).collect())
        }
    }

    fn validate_name(name: &str) -> String {
        name.chars().filter(|&c| validate_char(c)).collect()
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    pub fn set_name(&mut self, new_name: &str) {
        self.name = Self::validate_name(new_name);
    }

    #[inline]
    pub fn context(&self) -> Option<&str> {
        Some(self.context.as_ref()?.as_str())
    }

    #[inline]
    pub fn set_context(&mut self, new_context: &str) {
        self.context = Self::validate_context(new_context);
    }
}

fn validate_char(c: char) -> bool {
    match c {
        '\n' | '\t' | '\r' => false,
        _ => true,
    }
}
