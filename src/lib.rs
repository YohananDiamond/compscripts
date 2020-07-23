use std::fs::{create_dir_all, File, OpenOptions};
use std::path::Path;
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

pub fn touch_and_open(path: &Path) -> Result<File, String> {

    if path.exists() {
        if path.is_dir() {
            Err("path is a directory".into())
        } else {
            match OpenOptions::new().read(true).write(true).open(path) {
                Ok(f) => Ok(f),
                Err(e) => Err(format!("{}", e)),
            }
        }
    } else {
        if let Some(parent) = path.parent() {
            if parent.exists() {
                if parent.is_file() {
                    return Err(format!(
                        "parent path {} is not a directory",
                        parent.display()
                    ));
                }
            } else {
                if let Err(e) = create_dir_all(parent) {
                    return Err(format!(
                        "failed to create parent path {}: {}",
                        parent.display(),
                        e
                    ));
                }
            }
        }

        match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
        {
            Ok(f) => Ok(f),
            Err(e) => Err(format!("failed to create file: {}", e)),
        }
    }
}
