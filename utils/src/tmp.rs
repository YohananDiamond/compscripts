use rand::distributions::Alphanumeric;
use rand::Rng;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::process::Command;

use std::path::PathBuf;

pub fn make_tmp(extension: Option<&str>) -> PathBuf {
    loop {
        let path_str = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(6)
            .collect::<String>();

        let mut pathbuf = std::env::temp_dir();
        pathbuf.push(format!(
            "tmp.{}{}",
            path_str,
            match extension {
                Some(ext) => format!(".{}", ext),
                None => String::new(),
            }
        ));

        if !pathbuf.as_path().exists() {
            break pathbuf;
        }
    }
}

pub mod folder_lock {
    use std::io::{self, ErrorKind};
    use std::path::PathBuf;
    use std::fmt;

    #[derive(Debug)]
    pub enum LockError {
        InvalidLockName,
        AlreadyLocked,
        IoError(io::Error),
    }

    impl fmt::Display for LockError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::InvalidLockName => write!(f, "Invalid lock name"),
                Self::AlreadyLocked => write!(f, "Lock already exists (other instance of this application might be running)"),
                Self::IoError(err) => write!(f, "I/O error: {}", err),
            }
        }
    }

    #[derive(Debug)]
    pub enum ReleaseError {
        AlreadyReleased,
        IoError(io::Error),
    }

    pub struct FolderLock {
        lock_path: PathBuf,
        released: bool,
    }

    impl FolderLock {
        pub fn lock(lock_name: &str) -> Result<Self, LockError> {
            if lock_name.chars().any(|c| matches!(c, '/' | '\\')) {
                return Err(LockError::InvalidLockName);
            }

            let mut path = std::env::temp_dir();
            path.push(format!("{}.lock", lock_name));

            if let Err(e) = std::fs::create_dir(&path) {
                return Err(match e.kind() {
                    ErrorKind::AlreadyExists => LockError::AlreadyLocked,
                    _ => LockError::IoError(e),
                });
            }

            Ok(Self {
                lock_path: path,
                released: false,
            })
        }

        pub fn release(&mut self) -> Result<(), ReleaseError> {
            if let Err(e) = std::fs::remove_dir(&self.lock_path) {
                return Err(match e.kind() {
                    ErrorKind::NotFound => ReleaseError::AlreadyReleased,
                    _ => ReleaseError::IoError(e),
                });
            }

            self.released = true;
            Ok(())
        }
    }

    impl Drop for FolderLock {
        fn drop(&mut self) {
            if !self.released {
                match self.release() {
                    Ok(()) | Err(ReleaseError::AlreadyReleased) => {}
                    Err(other) => Err(other).expect("failed to release lock"),
                }
            }
        }
    }
}

pub fn make_folder_lock(
    lock_name: &str,
) -> Result<folder_lock::FolderLock, folder_lock::LockError> {
    folder_lock::FolderLock::lock(lock_name)
}

pub fn edit_text(text: &str, extension: Option<&str>) -> Result<(String, i32), String> {
    let tmpbuf = make_tmp(extension);

    {
        // touch temp file
        let mut tmpfile = match OpenOptions::new()
            .write(true)
            .create(true)
            .open(tmpbuf.as_path().to_str().unwrap())
        {
            Ok(file) => file,
            Err(e) => return Err(format!("failed to create temp file: {}", e)),
        };

        write!(tmpfile, "{}", text).unwrap();
    }

    // edit file
    let editor = std::env::var("MAYBE_GRAPHICAL_EDITOR")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "compscripts-defaultedit".into());

    let code = match Command::new(&editor)
        .args(&[tmpbuf.as_path().to_str().unwrap()])
        .spawn()
    {
        Ok(mut child) => child.wait().unwrap().code().unwrap_or(130),
        Err(why) => return Err(format!("failed to start process: {}", why)),
    };

    let mut buf = String::new();
    {
        // read new contents
        let mut tmpfile = match OpenOptions::new()
            .read(true)
            .open(tmpbuf.as_path().to_str().unwrap())
        {
            Ok(file) => file,
            Err(why) => return Err(format!("failed to create temp file: {}", why)),
        };

        tmpfile
            .read_to_string(&mut buf)
            .expect("failed to read buffer to string");
    }

    // remove the file, if it still exists
    let _ = std::fs::remove_file(tmpbuf.as_path());

    Ok((buf, code))
}
