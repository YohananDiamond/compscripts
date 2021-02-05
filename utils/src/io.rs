use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

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
            } else if let Err(e) = create_dir_all(parent) {
                return Err(format!(
                    "failed to create parent path {}: {}",
                    parent.display(),
                    e
                ));
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
        }
        Err(e) => Err(format!("failed to create file: {}", e)),
    }
}

pub fn read_line(prompt: &str) -> Result<String, io::Error> {
    eprint!("{}", prompt);
    io::stdout().flush().unwrap();

    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;

    Ok(buffer.trim().into())
}
