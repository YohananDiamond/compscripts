//! Stores data structures related to managing the database.

use std::collections::HashSet;
use std::path::Path;

use crate::item::{InternalId, Item, ItemState, RefId};

use core::data::data_serialize;

/// The core structure of the database.
pub struct ItemManager {
    /// The "root" of the data managed by this database. All items are contained here.
    pub data: Vec<Item>,
    /// A set that stores all the used internal IDs.
    /// TODO: consider removing this one. Simply having the greatest internal ID stored seems enough.
    internal_ids: HashSet<u32>,
    /// A set that stores all the used reference IDs.
    ref_ids: HashSet<u32>,
}

/// A collection of errors that can happen during the ItemManager creation.
pub enum ManagerError {
    /// At least two of the items have a repeated reference ID.
    RepeatedRefID(RefId),
    /// At least two of the items have a repeated internal ID.
    RepeatedInternalID(InternalId),
}

/// A trait to help on searching through a database with different types of queries.
pub trait Searchable<T> {
    /// The data possibly returned, in reference, by the search.
    type Data;

    /// Attempts to find `query`, returning an immutable reference to it if found.
    fn find(&self, query: T) -> Option<&Self::Data>;

    /// Attempts to find `query`, returning a mutable reference to it if found.
    fn find_mut(&mut self, query: T) -> Option<&mut Self::Data>;
}

/// An extension trait to [`Searchable<T>`], which allows the caller to find and interact with a single piece of data at
/// once, safely.
///
/// [`Searchable<T>`]: Searchable
pub trait Interactable<T>: Searchable<T> {
    /// Finds a piece of data by immutable reference with `query`, and runs `interaction` on it, returning the output
    /// `O` of the function.
    fn interact<O, F>(&self, query: T, interaction: F) -> Option<O>
    where
        F: FnOnce(&<Self as Searchable<T>>::Data) -> O,
    {
        Some(interaction(self.find(query)?))
    }

    /// Finds a piece of data by mutable reference with `query`, and runs `interaction` on it, returning the output `O`
    /// of the function.
    fn interact_mut<O, F>(&mut self, query: T, interaction: F) -> Option<O>
    where
        F: FnOnce(&mut <Self as Searchable<T>>::Data) -> O,
    {
        Some(interaction(self.find_mut(query)?))
    }
}

impl<T, M> Interactable<T> for M where M: Searchable<T> {}

impl Searchable<RefId> for ItemManager {
    type Data = Item;

    fn find(&self, query: RefId) -> Option<&Item> {
        fn search(items: &Vec<Item>, query: RefId) -> Option<&Item> {
            for item in items {
                if item.ref_id == Some(query.0) {
                    return Some(item);
                }

                let find_result = search(&item.children, query);
                if find_result.is_some() {
                    return find_result;
                }
            }

            None
        }

        search(&self.data, query)
    }

    fn find_mut(&mut self, query: RefId) -> Option<&mut Item> {
        fn search(items: &mut Vec<Item>, query: RefId) -> Option<&mut Item> {
            for item in items {
                if item.ref_id == Some(query.0) {
                    return Some(item);
                }

                let find_result = search(&mut item.children, query);
                if find_result.is_some() {
                    return find_result;
                }
            }

            None
        }

        search(&mut self.data, query)
    }
}

impl Searchable<InternalId> for ItemManager {
    type Data = Item;

    fn find(&self, query: InternalId) -> Option<&Item> {
        fn search(items: &Vec<Item>, query: InternalId) -> Option<&Item> {
            for item in items {
                if item.internal_id == query.0 {
                    return Some(item);
                }

                let find_result = search(&item.children, query);
                if find_result.is_some() {
                    return find_result;
                }
            }

            None
        }

        search(&self.data, query)
    }

    fn find_mut(&mut self, query: InternalId) -> Option<&mut Item> {
        fn search(items: &mut Vec<Item>, query: InternalId) -> Option<&mut Item> {
            for item in items {
                if item.internal_id == query.0 {
                    return Some(item);
                }

                let find_result = search(&mut item.children, query);
                if find_result.is_some() {
                    return find_result;
                }
            }

            None
        }

        search(&mut self.data, query)
    }
}

/// The result returned by a program.
pub struct ProgramResult {
    pub should_save: bool,
    pub exit_status: i32,
}

impl ItemManager {
    /// Attempts to create an ItemManager instance, returning a [`ManagerError`] if the operation failed.
    ///
    /// [`ManagerError`]: ManagerError
    pub fn new(mut data: Vec<Item>) -> Result<Self, ManagerError> {
        let mut ref_set: HashSet<u32> = HashSet::new();
        let mut in_set: HashSet<u32> = HashSet::new();

        fn travel(
            data: &Vec<Item>,
            ref_set: &mut HashSet<u32>,
            in_set: &mut HashSet<u32>,
        ) -> Result<(), ManagerError> {
            // TODO: store the amount of items to reserve on a file instead of doing this
            ref_set.reserve(data.len() / 4); // reserve a fraction of the amount of items, since many of them might not have ref IDs.
            in_set.reserve(data.len());

            for item in data {
                // add RefID
                if let Some(id) = item.ref_id {
                    if ref_set.contains(&id) {
                        return Err(ManagerError::RepeatedRefID(RefId(id)));
                    } else {
                        ref_set.insert(id);
                    }
                }

                // add InternalID
                if in_set.contains(&item.internal_id) {
                    return Err(ManagerError::RepeatedInternalID(InternalId(
                        item.internal_id,
                    )));
                } else {
                    in_set.insert(item.internal_id);
                }

                if !item.children.is_empty() {
                    travel(&item.children, ref_set, in_set)?;
                }
            }

            Ok(())
        }

        travel(&data, &mut ref_set, &mut in_set)?;

        // With the now filled IDs set, find free reference IDs for pending/note items that don't have IDs.
        for item in data.iter_mut() {
            match item.state {
                ItemState::Done => (),
                ItemState::Todo | ItemState::Note => {
                    if item.ref_id.is_none() {
                        let id = core::misc::find_lowest_free_value(&ref_set);
                        item.ref_id = Some(id);
                        ref_set.insert(id);
                    }
                }
            }
        }

        Ok(Self {
            ref_ids: ref_set,
            internal_ids: in_set,
            data: data,
        })
    }

    /// Starts a program of function signature F, which takes a mutable reference of the manager as an argument and
    /// returns a ProgramResult struct.
    pub fn start_program_with_file<F>(&mut self, file: &Path, program: F) -> i32
    where
        F: FnOnce(&mut ItemManager) -> ProgramResult,
    {
        let result = program(self);

        if result.should_save {
            if let Err(e) = data_serialize::save_to_file(&self.data, file, true) {
                eprintln!("Error: failed to save to file: {}", e);
                return 1;
            }
        }

        result.exit_status
    }

    /// Constructs and adds an item to the root of the database.
    ///
    /// Returns the item's RefId.
    pub fn add_item_on_root(
        &mut self,
        name: &str,
        context: &str,
        state: ItemState,
        description: String,
        children: Vec<Item>,
    ) -> RefId {
        // Might crash with an overflow but seriously, who is gonna have 4,294,967,296 items in a lifetime?
        let free_ref_id = core::misc::find_lowest_free_value(&self.ref_ids);
        self.ref_ids.insert(free_ref_id);

        let free_internal_id = core::misc::find_highest_free_value(&self.internal_ids);
        self.internal_ids.insert(free_internal_id);

        self.data.push(Item::new(
            Some(free_ref_id),
            free_internal_id,
            name,
            context,
            state,
            description,
            children,
        ));

        RefId(free_ref_id)
    }

    pub fn add_child<Q>(
        &mut self,
        query: Q,
        name: &str,
        context: &str,
        state: ItemState,
        description: String,
        children: Vec<Item>,
    ) -> Result<RefId, ()>
    where
        Self: Searchable<Q, Data = Item>,
    {
        let free_ref_id = core::misc::find_lowest_free_value(self.ref_ids());
        self.ref_ids.insert(free_ref_id);

        let free_internal_id = core::misc::find_highest_free_value(self.internal_ids());
        self.internal_ids.insert(free_internal_id.into());

        if let Some(i) = self.find_mut(query) {
            i.children.push(Item::new(
                Some(free_ref_id),
                free_internal_id,
                name,
                context,
                state,
                description,
                children,
            ));

            Ok(RefId(free_ref_id))
        } else {
            Err(())
        }
    }

    pub fn surface_ref_ids(&self) -> Vec<RefId> {
        self.data
            .iter()
            .filter_map(|i| i.ref_id.and_then(|id| Some(RefId(id))))
            .collect()
    }

    // pub fn get_all_ref_ids(&self) -> Vec<RefId> {}

    pub fn try_remove(&mut self, ref_id: RefId) -> Option<Item> {
        fn search(items: &mut Vec<Item>, ref_id: RefId) -> Option<Item> {
            let mut i = 0;

            while i < items.len() {
                if items[i].ref_id == Some(ref_id.0) {
                    // FIXME: should this really be O(n)?
                    return Some(items.remove(i));
                }

                if let Some(item) = search(&mut items[i].children, ref_id) {
                    return Some(item);
                }

                i += 1;
            }

            None
        }

        search(&mut self.data, ref_id)
    }

    pub fn first_invalid_ref_id<'a, I>(&self, ids: I) -> Option<RefId>
    where
        I: Iterator<Item = &'a u32>,
    {
        for id in ids {
            let ref_id = RefId(*id);
            if self.find(ref_id).is_none() {
                return Some(ref_id);
            }
        }

        None
    }

    pub fn swap<T, E>(&mut self, query_1: T, query_2: E) -> Result<(), String>
    where
        Self: Searchable<T, Data = Item> + Searchable<E, Data = Item>,
    {
        unsafe {
            // This unsafe block simply swaps two items. I don't
            // think there is any issue with this.

            // try to get first item
            let first: *mut Item = match self.find_mut(query_1) {
                Some(m) => m,
                None => return Err(format!("first query could not be found")),
            };

            // try to get first item
            let second: *mut Item = match self.find_mut(query_2) {
                Some(m) => m,
                None => return Err(format!("second query could not be found")),
            };

            // check if swap is needed and do the thing
            if first != second {
                std::ptr::swap(first, second);
                Ok(())
            } else {
                Err(format!("first and second queries are the same item"))
            }
        }
    }

    pub fn change_item_state<Q, F>(&mut self, id: Q, mapper: F) -> Result<(), ()>
    where
        Self: Searchable<Q, Data = Item>,
        F: FnOnce(ItemState) -> ItemState,
    {
        let item = self.find_mut(id).ok_or(())?;
        let new_state = mapper(item.state);

        if new_state == ItemState::Done {
            item.ref_id = None;
        }

        item.state = new_state;

        Ok(())
    }
}

impl ItemManager {
    #[inline(always)]
    pub fn internal_ids(&self) -> &HashSet<u32> {
        &self.internal_ids
    }

    #[inline(always)]
    pub fn ref_ids(&self) -> &HashSet<u32> {
        &self.ref_ids
    }
}
