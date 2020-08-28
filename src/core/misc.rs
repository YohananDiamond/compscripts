use regex::Regex;
use std::cmp::Eq;
use std::collections::HashSet;
use std::hash::Hash;
use std::io::{Read, Write};
use std::process::{Command, Stdio, Termination};
use std::fmt::Display;

#[derive(Clone, Copy)]
pub struct ExitCode(pub i32);

impl Termination for ExitCode {
    fn report(self) -> i32 {
        self.0
    }
}

pub enum ExitResult<'a> {
    Success,
    SilentError,
    DisplayError(Box<dyn Display + 'a>),
}

impl<T: Display> From<Result<(), T>> for ExitCode {
    fn from(r: Result<(), T>) -> ExitCode {
        if let Err(e) = r {
            eprintln!("Error: {}", e);
            ExitCode(1)
        } else {
            ExitCode(0)
        }
    }
}

impl From<ExitResult<'_>> for ExitCode {
    fn from(r: ExitResult) -> ExitCode {
        match r {
            ExitResult::Success => ExitCode(0),
            ExitResult::SilentError => ExitCode(1),
            ExitResult::DisplayError(e) => {
                eprintln!("Error: {}", e);
                ExitCode(1)
            }
        }
    }
}

impl<'a, T: Display + 'a> From<T> for ExitResult<'a> {
    fn from(thing: T) -> ExitResult<'a> {
        ExitResult::DisplayError(Box::new(thing))
    }
}

pub fn fzagnostic(prompt: &str, input: &str, height: u32) -> Option<String> {
    match Command::new("fzagnostic")
        .args(&["-h", &format!("{}", height), "-p", prompt])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            let stdin = child.stdin.as_mut().unwrap();
            write!(stdin, "{}", input).unwrap();

            if child.wait().unwrap().code().unwrap() == 0 {
                let mut choice = String::new();
                match child.stdout.as_mut().unwrap().read_to_string(&mut choice) {
                    Ok(_) => Some(choice),
                    Err(_) => None,
                }
            } else {
                None
            }
        }
        Err(_) => {
            eprintln!("failed to run command");
            None
        }
    }
}

pub fn find_free_value(set: &HashSet<u32>) -> u32 {
    let mut free_value = 0;
    loop {
        if !set.contains(&free_value) {
            break free_value;
        }
        free_value += 1;
    }
}

pub fn parse_range_str(string: &str) -> Result<Vec<u32>, String> {
    let mut result: Vec<u32> = Vec::new();
    let range_regex = Regex::new(r"^(\d+)\.\.(\d+)$").unwrap();
    let number_regex = Regex::new(r"^\d+$").unwrap();

    for number in string
        .chars()
        .filter(|x| *x != ' ')
        .collect::<String>()
        .split(',')
    {
        if number_regex.is_match(number) {
            result.push(number.parse::<u32>().unwrap())
        } else if range_regex.is_match(number) {
            let captures = range_regex.captures(number).unwrap();
            let num1: u32 = captures[1].parse().unwrap();
            let num2: u32 = captures[2].parse().unwrap();

            if num2 < num1 {
                return Err(format!(
                    "Second number {} is smaller than first number {} in range {}",
                    num2, num1, number
                ));
            }

            let mut i: u32 = num1;
            loop {
                result.push(i);
                i += 1;
                if i > num2 {
                    break;
                }
            }
        } else {
            return Err(format!("Could not parse {:?}", number));
        }
    }

    Ok(result)
}

/// Get the first item from a slice not on a set.
pub fn get_first_not_on_set<'a, T: Hash + Eq>(
    selection: &'a [T],
    set: &HashSet<T>,
) -> Option<&'a T> {
    for s in selection {
        if !set.contains(s) {
            return Some(s);
        }
    }

    None
}

pub fn confirm_with_default(default: bool) -> bool {
    loop {
        let input = crate::io::read_line(&format!(
            "Confirm? [{}] ",
            if default { "Y/n" } else { "y/N" }
        ))
        .unwrap();

        match input.as_str() {
            "" => break default,
            "y" | "Y" => break true,
            "n" | "N" => break false,
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range() {
        let range_str = "1..10,4,5";
        assert_eq!(
            parse_range_str(range_str),
            Ok(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 4, 5])
        );
    }
}
