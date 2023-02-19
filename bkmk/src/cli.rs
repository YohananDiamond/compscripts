use clap::Parser;

#[derive(Parser)]
pub struct Options {
    #[arg(
        short,
        long,
        help = "the path to the bookmarks file (default: $BKMN_FILE -> ~/.local/share/bkmk)"
    )]
    pub path: Option<String>,

    #[command(subcommand)]
    pub subcmd: SubCmd,
}

#[derive(Parser)]
pub enum SubCmd {
    #[command(about = "adds an URL to the bookmarks list")]
    Add(AddParameters),

    #[command(about = "adds the URLs from a newline-delimited bookmarks list file")]
    AddFromFile(FileParameters),

    #[command(about = "opens an interactive menu for managing bookmarks using fzagnostic")]
    Menu,
}

#[derive(Parser)]
pub struct AddParameters {
    #[arg(help = "the URL of the bookmark")]
    pub url: String,

    #[arg(short, long, help = "the title of the bookmark")]
    pub title: Option<String>,
}

#[derive(Parser)]
pub struct FileParameters {
    pub file: String,
}
