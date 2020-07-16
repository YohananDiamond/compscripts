use std::io::{Read, Write};
use std::process::{Stdio, Command};

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
