#![allow(unreachable_code)]

use clap::Parser;

use std::borrow::Cow;

mod cli;

use utils::error::ExitCode;
// use utils::misc::confirm_with_default;

const PREFIX_LINES: &[&str] = &[
    "# This is a comment.",
    "# Beware! Only lines what start with # are comments.",
    "# Lines with a # after the first character are not comments.",
];

fn main() -> ExitCode {
    let _options = cli::PreOptions::parse().process();

    // decide upon what to rename
    panic!();

    let mut input: Vec<Cow<'_, str>> = PREFIX_LINES
        .iter()
        .cloned()
        .map(|line: &str| Cow::Borrowed(line))
        .collect();

    input.extend(["a"].iter().enumerate().map(|(_, _)| Cow::Owned(panic!()))); // extend with the rename list's IDs

    let output = utils::tmp::edit_text(&input.join("\n"), Some("txt")).unwrap();

    let _useful_output = output
        .0
        .split("\n")
        .filter(|line| line.chars().nth(0) != Some('#'))
        .filter(|line| !line.is_empty());

    ExitCode::new(130)
}
