use clap::Clap;

#[derive(Clap)]
pub struct Options {
    #[clap(short, long, about = "the path to the bookmarks file (default: $BKMN_FILE -> ~/.local/share/bkmk)")]
    pub path: Option<String>,
    #[clap(subcommand)]
    pub subcmd: SubCmd,
}

#[derive(Clap)]
pub enum SubCmd {
    #[clap(about = "adds an URL to the bookmarks list")]
    Add(AddParameters),
    #[clap(about = "adds the URLs from a newline-delimited bookmarks list file")]
    AddFromFile(FileParameters),
    #[clap(about = "opens an interactive menu for managing bookmarks using fzagnostic")]
    Menu,
}

#[derive(Clap)]
pub struct AddParameters {
    #[clap(about = "the URL of the bookmark")]
    pub url: String,
    #[clap(short, long, about = "the title of the bookmark")]
    pub title: Option<String>,
}

#[derive(Clap)]
pub struct FileParameters {
    pub file: String,
}
