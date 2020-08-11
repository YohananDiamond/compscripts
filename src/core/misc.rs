use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::collections::HashSet;

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
    let mut free_value = 0u32;
    loop {
        if !set.contains(&free_value) {
            break free_value;
        }
        free_value += 1;
    }
}
