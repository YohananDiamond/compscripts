use std::path::Path;
use std::io;
use crate::aliases::JsonError;
use serde::{Deserialize, Serialize};

pub trait JsonArraySerializer<'a, T> {
    fn export(&self) -> String;
    fn import(string: &'a str) -> Result<Vec<T>, JsonError>;

    fn save_to_file(&mut self, file: &Path) -> Result<(), io::Error> {
        let compiled_string = self.export();
        std::fs::write(file, &compiled_string)
    }
}

pub struct ArrayLines<'a, T>
where
    T: Deserialize<'a> + Serialize,
{
    data: &'a [T],
}

impl<'a, T> ArrayLines<'a, T>
where
    T: Deserialize<'a> + Serialize,
{
    pub fn new(arr: &'a [T]) -> Self {
        Self {
            data: arr,
        }
    }
}

impl<'a, T> JsonArraySerializer<'a, T> for ArrayLines<'a, T>
where
    T: Deserialize<'a> + Serialize,
{
    fn export(&self) -> String {
        self.data.iter()
            .map(|x| serde_json::to_string(x).unwrap())
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn import(string: &'a str) -> Result<Vec<T>, JsonError> {
        string
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
}

pub struct ArrayArray<'a, T>
where
    T: Deserialize<'a> + Serialize,
{
    data: &'a [T],
}

impl<'a, T> ArrayArray<'a, T>
where
    T: Deserialize<'a> + Serialize,
{
    pub fn new(arr: &'a [T]) -> Self {
        Self {
            data: arr,
        }
    }
}

impl<'a, T> JsonArraySerializer<'a, T> for ArrayArray<'a, T>
where
    T: Deserialize<'a> + Serialize,
{
    fn export(&self) -> String {
        serde_json::to_string(self.data).unwrap()
    }

    fn import(string: &'a str) -> Result<Vec<T>, JsonError> {
        serde_json::from_str(string)
    }
}
