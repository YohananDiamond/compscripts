use regex::Regex;
use std::cmp::Eq;
use std::collections::HashSet;
use std::hash::Hash;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::error::CliError;

/// Runs the `fzagnostic` command with data from the arguments.
///
/// Returns Ok with the choice if everything went successfully.
///
/// Returns Err with the error if the error was not intended.
/// Returns Err with an empty string if fzagnostic was cancelled manually. (Ctrl-C, ESC etc.)
pub fn fzagnostic<'a, C, S>(prompt: &str, choices: C, height: u32) -> Result<String, CliError>
where
    C: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match Command::new("fzagnostic")
        .args(&["-h", &format!("{}", height), "-p", prompt])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            let stdin = child.stdin.as_mut().unwrap();

            for line in choices.into_iter() {
                write!(stdin, "{}\n", line.as_ref()).map_err(|why| {
                    CliError::from_display(format!(
                        "fzagnostic: failed to write to process stdin: {}",
                        why
                    ))
                })?;
            }

            if child.wait().unwrap().success() {
                let mut choice = String::new();
                match child.stdout.as_mut().unwrap().read_to_string(&mut choice) {
                    Ok(_) => Ok(choice),
                    Err(why) => Err(CliError::from_display(format!(
                        "fzagnostic: failed to get process stdout: {}",
                        why
                    ))),
                }
            } else {
                Err(CliError::Silent)
            }
        }
        Err(why) => Err(CliError::from_display(format!(
            "fzagnostic: failed to run command: {}",
            why
        ))),
    }
}

/// Finds the first free value in the set.
pub fn find_lowest_free_value(set: &HashSet<u32>) -> u32 {
    let mut free_value = 0;
    loop {
        if !set.contains(&free_value) {
            break free_value;
        }
        free_value += 1;
    }
}

/// Finds the first free value that is bigger than the highest used value in the set.
pub fn find_highest_free_value(set: &HashSet<u32>) -> u32 {
    let free_value = set.iter().fold(0, |x, &y| x.max(y));

    if set.contains(&free_value) {
        free_value + 1
    } else {
        free_value
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
        .unwrap()
        .to_lowercase();

        match input.as_str() {
            "" => break default,
            "y" | "yes" => break true,
            "n" | "no" => break false,
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
