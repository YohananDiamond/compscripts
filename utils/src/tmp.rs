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
    // TODO: better customization of this option (EDITOR does not necessarily handle both the GUI and TTY cases)
    let editor = std::env::var("EDITOR").unwrap_or("compscripts-defaultedit".into());
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

        tmpfile.read_to_string(&mut buf).expect("failed to read buffer to string");
    }

    // remove the file, if it still exists
    let _ = std::fs::remove_file(tmpbuf.as_path());

    Ok((buf, code))
}
