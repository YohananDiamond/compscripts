use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
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

pub fn touch_read(path: &Path) -> Result<String, String> {
    match touch_and_open(path) {
        Ok(mut f) => {
            let mut contents = String::new();
            if let Err(e) = f.read_to_string(&mut contents) {
                Err(format!("failed to read file buffer: {}", e))
            } else {
                Ok(contents)
            }
        },
        Err(e) => {
            Err(format!("failed to create file: {}", e))
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
