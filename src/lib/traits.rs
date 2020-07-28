use crate::lib::JsonError;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

pub trait DataManager {
    type Data: Ord + PartialOrd;

    /// Starts the main program for the manager.
    /// Returns an integer that should be interpreted as the application exit code.
    fn start() -> i32;

    /// Returns an immutable reference to the data inside the manager.
    fn data(&self) -> &Vec<Self::Data>;

    /// Returns a mutable reference to the data inside the manager.
    fn data_mut(&mut self) -> &mut Vec<Self::Data>;
}

pub trait JsonLines<'a>: DataManager
where
    <Self as DataManager>::Data: Deserialize<'a> + Serialize,
{
    fn into_json_lines(&self) -> String {
        self.data()
            .iter()
            .map(|x| serde_json::to_string(x).unwrap())
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn from_json_lines(lines: &'a str) -> Result<Vec<Self::Data>, JsonError> {
        lines
            .split("\n")
            .filter_map(|line| {
                if line.len() == 0 {
                    None
                } else {
                    Some(serde_json::from_str(line))
                }
            })
            .collect()
    }

    fn save_to_file(&mut self, file: &Path) -> Result<(), io::Error> {
        self.data_mut().sort();
        let compiled_string = self.into_json_lines();
        std::fs::write(file, &compiled_string)
    }
}
