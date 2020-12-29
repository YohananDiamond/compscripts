//! Stores data structures related to displaying the database on a terminal.

// TODO: add a way to recursively sort items, like what was done with filters.

use crate::item::{Item, ItemState};
use core::cowstr::CowStr;

use std::io;
use std::io::Write;

#[derive(Clone, Copy)]
/// Specifies the way the items should be shown on the screen.
pub enum ReportDepth {
    /// Only show the item itself.
    Shallow,
    /// Show the item, the first child (if any) and a message saying how many more children are there (if any).
    Brief,
    /// Show all children of an item.
    Tree,
}

// #[derive(Clone, Copy)]
// pub enum SortOption {
//     Default,
//     Alphabetical,
// }

// impl Default for SortOption {
//     fn default() -> Self {
//         Self::Default
//     }
// }

/// Stores settings for the report displaying.
#[derive(Clone)]
pub struct ReportConfig {
    /// The amount of spaces used per indent.
    pub spaces_per_indent: usize,
}

impl ReportConfig {
    pub fn get_indent_spaces(&self, indent: usize) -> String {
        std::iter::repeat(' ')
            .take(self.spaces_per_indent * indent)
            .collect()
    }
}

#[derive(Clone)]
pub struct ReportInfo<'a> {
    /// An immutable reference to the report config.
    pub config: &'a ReportConfig,
    /// The indent level. Not the same as the final indent, or the amount of spaces per indent.
    pub indent: usize,
    /// The filter that the items must go through to be printed, if any.
    pub filter: Option<&'a dyn Fn(&Item) -> bool>,
    /// The depth that the item displaying must go through.
    pub depth: ReportDepth,
    // pub sort: SortOption,
}

pub trait Report {
    fn display(item: &Item, info: &ReportInfo, out: &mut dyn Write) -> io::Result<()>;
    fn display_all(
        items: &mut dyn Iterator<Item = &Item>,
        info: &ReportInfo,
        out: &mut dyn Write,
    ) -> io::Result<()>;
    fn report(
        label: &str,
        items: &mut dyn Iterator<Item = &Item>,
        info: &ReportInfo,
        out: &mut dyn Write,
    ) -> io::Result<()> {
        let length_message: CowStr = match items.size_hint().0 {
            0 | 1 => "No items to be displayed".into(),
            2 => "1 item to be displayed".into(),
            i => format!("{} items to be displayed", i - 1).into(),
        };

        writeln!(out, "{} | {}", label, length_message)?;

        Self::display_all(items, info, out)
    }
}

pub struct BasicReport;
impl Report for BasicReport {
    fn display(item: &Item, info: &ReportInfo, out: &mut dyn Write) -> io::Result<()> {
        let proceed = |out: &mut dyn Write| -> io::Result<()> {
            writeln!(
                out,
                "{indent}{state} {text} {context}{id_repr}{flags}",
                indent = info.config.get_indent_spaces(info.indent),
                state = match item.state {
                    ItemState::Todo => "o",
                    ItemState::Done => "x",
                    ItemState::Note => "-",
                },
                context = match item.context() {
                    Some(ctx) => format!("@{} ", ctx),
                    None => String::new(),
                },
                text = item.name,
                id_repr = match item.ref_id {
                    Some(id) => format!("#{:>02}", id),
                    None => format!("i{:>02}", item.internal_id),
                },
                flags = match item.description.is_empty() {
                    true => "",
                    false => " (D)",
                },
            )?;

            match info.depth {
                ReportDepth::Shallow => (),
                ReportDepth::Brief => {
                    let mut info = info.clone();
                    info.indent += 1;
                    info.depth = ReportDepth::Shallow;

                    if item.children.len() > 0 {
                        Self::display(&item.children[0], &info, out)?;

                        if item.children.len() > 1 {
                            writeln!(
                                out,
                                "{}  {} more...",
                                info.config.get_indent_spaces(info.indent),
                                item.children.len() - 1
                            )?;
                        }
                    }
                }
                ReportDepth::Tree => {
                    let mut info = info.clone();
                    info.indent += 1;

                    Self::display_all(&mut item.children.iter(), &info, out)?;
                }
            }

            Ok(())
        };

        if let Some(filter) = info.filter {
            if filter(item) {
                proceed(out)?;
            }
        } else {
            proceed(out)?;
        }

        Ok(())
    }

    fn display_all(
        items: &mut dyn Iterator<Item = &Item>,
        info: &ReportInfo,
        out: &mut dyn Write,
    ) -> io::Result<()> {
        for item in items {
            Self::display(item, info, out)?;
        }

        Ok(())
    }
}
