use clap::Parser;

#[derive(Debug, Parser)]
pub struct PreOptions {
    #[arg(
        help = "the list of files to rename: if empty, defaults to current directory; if it has only one element, renames the contents of the directory, if it is a directory, or just the file's name, if it is a file or the --as-file option is set; if it has more than one element, treat it as a list of files"
    )]
    pub files: Vec<String>,

    #[arg(short, long, help = "show every operation done")]
    pub verbose: Option<bool>,

    #[arg(
        short,
        long,
        help = "ignore errors; if false will stop the entire process if any error occurs",
    )]
    pub ignore_errors: Option<bool>,

    #[arg(
        short,
        long,
        help = "show the prefix numbers and error if they are out of order after editing (recommended)",
    )]
    pub prefix_numbers: Option<bool>,

    #[arg(
        short,
        long,
        help = "treat FILES as a list of things to rename - only has noticeable effect on a one-element list with a directory",
    )]
    pub as_file: Option<bool>,

    #[arg(
        short,
        long,
        help = "show each file's full path",
    )]
    pub full_path: Option<bool>,

    // TODO: keep failed list at /tmp (or prompt to re-edit again)
}

pub struct Options {
    pub files: Vec<String>,
    pub verbose: bool,
    pub ignore_errors: bool,
    pub prefix_numbers: bool,
    pub as_file: bool,
    pub full_path: bool,
}

impl PreOptions {
    pub fn process(self) -> Options {
        Options {
            files: self.files,
            verbose: self.verbose.unwrap_or(true),
            ignore_errors: self.ignore_errors.unwrap_or(false),
            prefix_numbers: self.prefix_numbers.unwrap_or(true),
            as_file: self.as_file.unwrap_or(false),
            full_path: self.full_path.unwrap_or(false),
        }
    }
}
