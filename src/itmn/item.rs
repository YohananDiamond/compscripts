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
    /// The name of the Item.
    pub name: String,
    /// The context of the item, GTD-style.
    pub context: Option<String>,
    /// The state of the item (note, todo, done etc.)
    pub state: State,
    /// The children of this item.
    /// TODO: optimize this with Option<Vec> for less unecessary allocations
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
    pub fn normalize(self) -> Self {
        let new_name = self.name.chars().filter(|&c| c != '\n').collect();
        let new_context = if let Some(ctx) = self.context {
            Some(ctx.chars().filter(|&c| c == '\n').collect())
        } else {
            None
        };

        Self {
            name: new_name,
            context: new_context,
            ..self
        }
    }
}
