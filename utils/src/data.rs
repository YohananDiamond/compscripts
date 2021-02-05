use serde::{Deserialize, Serialize};
use std::io;
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

    /// Returns an immutable reference to the data inside the manager.
    fn data(&self) -> &[Self::Data];

    /// Returns a mutable reference to the data inside the manager.
    fn data_mut(&mut self) -> &mut Vec<Self::Data>;

    /// TODO: @doc
    fn find(&self, ref_id: Id) -> Option<&Self::Data> {
        self.data().iter().find(|i| i.ref_id() == Some(ref_id))
    }

    /// TODO: @doc
    fn find_mut(&mut self, ref_id: Id) -> Option<&mut Self::Data> {
        self.data_mut()
            .iter_mut()
            .find(|i| i.ref_id() == Some(ref_id))
    }

    /// Interacts with an item by its reference ID.
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

    /// TODO: @doc
    fn after_interact_mut_hook(&mut self);
}

pub mod data_serialize {
    use std::io;
    use std::path::Path;

    use super::{Deserialize, JsonError, Serialize};

    pub fn import<'a, T>(string: &'a str) -> Result<Vec<T>, JsonError>
    where
        T: Deserialize<'a> + Serialize,
    {
        serde_json::from_str(string)
    }

    pub fn export<'a, T>(data: &'a [T], prettified: bool) -> String
    where
        T: Deserialize<'a> + Serialize,
    {
        if prettified {
            // TODO: see if this unwrap is really safe
            serde_json::to_string_pretty(data).unwrap()
        } else {
            // TODO: see if this unwrap is really safe
            serde_json::to_string(data).unwrap()
        }
    }

    pub fn save_to_file<'a, T>(
        data: &'a [T],
        file: &'a Path,
        prettified: bool,
    ) -> Result<(), io::Error>
    where
        T: Deserialize<'a> + Serialize,
    {
        let export_string = export(data, prettified);
        std::fs::write(file, &export_string)
    }
}

pub trait JsonSerializer<'a>: Manager
where
    <Self as Manager>::Data: Deserialize<'a> + Serialize,
{
    /// TODO: @doc
    fn export(&self, prettified: bool) -> String {
        if prettified {
            serde_json::to_string_pretty(self.data()).unwrap()
        } else {
            serde_json::to_string(self.data()).unwrap()
        }
    }

    /// TODO: @doc
    fn import(string: &'a str) -> Result<Vec<Self::Data>, JsonError> {
        serde_json::from_str(string)
    }

    /// TODO: @doc
    fn save_to_file(&self, file: &Path, prettified: bool) -> Result<(), io::Error> {
        let export = self.export(prettified);
        std::fs::write(file, &export)
    }
}

impl<'a, T: Manager> JsonSerializer<'a> for T
where
    T: Manager,
    <T as Manager>::Data: Deserialize<'a> + Serialize,
{
}

// TODO: maybe make tests
