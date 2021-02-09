use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::aliases::JsonError;

/// The universal ID type.
/// Seems like a reasonable size for medium amounts of data.
pub type Id = u32;

/// A trait that describes data that can be searched.
pub trait Searchable: Ord + PartialOrd {
    /// Returns the reference ID of a data item.
    /// If None is returned, the data item is hidden (should be ignored).
    fn ref_id(&self) -> Option<Id>;
}

/// A trait for managing searchable data.
pub trait Manager {
    /// The searchable data type used on this manager.
    type Data: Searchable;

    /// Get an immutable reference to the data inside the manager.
    fn data(&self) -> &[Self::Data];

    /// Get a mutable reference to the data inside the manager.
    fn data_mut(&mut self) -> &mut Vec<Self::Data>;

    /// Find an instance of the item via its reference ID and return an immutable reference to it.
    fn find(&self, ref_id: Id) -> Option<&Self::Data> {
        self.data().iter().find(|i| i.ref_id() == Some(ref_id))
    }

    /// Find an instance of the item via its reference ID and return a mutable reference to it.
    fn find_mut(&mut self, ref_id: Id) -> Option<&mut Self::Data> {
        self.data_mut()
            .iter_mut()
            .find(|i| i.ref_id() == Some(ref_id))
    }

    /// Interact with an item by its reference ID.
    fn interact<T, F: Fn(&Self::Data) -> T>(&self, ref_id: Id, interaction: F) -> Option<T> {
        let item = self.data().iter().find(|i| i.ref_id() == Some(ref_id))?;
        Some(interaction(item))
    }

    /// Interacts with an item by its reference ID, possibly mutating it.
    fn interact_mut<T, F: Fn(&mut Self::Data) -> T>(
        &mut self,
        ref_id: Id,
        interaction: F,
    ) -> Option<T> {
        let item = self
            .data_mut()
            .iter_mut()
            .find(|i| i.ref_id() == Some(ref_id))?;

        let result = interaction(item);
        self.after_interact_mut_hook();
        Some(result)
    }

    /// A hook that is ran after a mutable interaction is made.
    fn after_interact_mut_hook(&mut self);
}

pub mod data_serialize {
    use std::path::Path;

    use super::{Deserialize, JsonError, Serialize};

    pub enum SaveToFileError {
        Saving(std::io::Error),
        Exporting(serde_json::Error),
    }

    impl std::fmt::Display for SaveToFileError {
        fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Saving(e) => write!(fmt, "Error while saving: {}", e),
                Self::Exporting(e) => write!(fmt, "Error while exporting: {}", e),
            }
        }
    }

    /// Import a vector of T from a json string.
    pub fn import<'a, T>(string: &'a str) -> Result<Vec<T>, JsonError>
    where
        T: Deserialize<'a> + Serialize,
    {
        serde_json::from_str(string)
    }

    /// Export a T slice into a json string.
    pub fn export<'a, T>(data: &'a [T], prettified: bool) -> serde_json::Result<String>
    where
        T: Deserialize<'a> + Serialize,
    {
        if prettified {
            serde_json::to_string_pretty(data)
        } else {
            serde_json::to_string(data)
        }
    }

    /// Export a T slice into a json string and then save it into a file.
    pub fn save_to_file<'a, T>(
        data: &'a [T],
        file: &'a Path,
        prettified: bool,
    ) -> Result<(), SaveToFileError>
    where
        T: Deserialize<'a> + Serialize,
    {
        let export_string = export(data, prettified).map_err(|e| SaveToFileError::Exporting(e))?;
        std::fs::write(file, &export_string).map_err(|e| SaveToFileError::Saving(e))?;

        Ok(())
    }
}

/// A trait for exporting data to json.
pub trait JsonSerializer<'a>: Manager
where
    <Self as Manager>::Data: Deserialize<'a> + Serialize,
{
    /// Export the data into a json-formatted string.
    fn export(&'a self, prettified: bool) -> serde_json::Result<String> {
        data_serialize::export(self.data(), prettified)
    }

    /// Import the data from a json-formatted string.
    fn import(string: &'a str) -> Result<Vec<Self::Data>, JsonError> {
        data_serialize::import(string)
    }

    /// Export the data to json and save it to a file.
    fn save_to_file(
        &'a self,
        file: &'a Path,
        prettified: bool,
    ) -> Result<(), data_serialize::SaveToFileError> {
        data_serialize::save_to_file(self.data(), file, prettified)
    }
}

impl<'a, M> JsonSerializer<'a> for M
where
    M: Manager,
    <M as Manager>::Data: Deserialize<'a> + Serialize,
{
}
