//! Stores data structures related to displaying the database on a terminal.

// TODO: add a way to recursively sort items, like what was done with filters.

use crate::item::{Item, ItemState};

#[derive(Clone, Copy)]
/// Specifies the way the items should be shown on the screen.
pub enum ReportStyle {
    /// Only show the item itself.
    Shallow,
    /// Show the item, the first child (if any) and a message saying how many more children are there (if any).
    Brief,
    /// Show all children of an item.
    Tree,
}

/// Stores settings for the displaying and manages the displaying itself.
pub struct ReportManager {
    pub spaces_per_indent: usize,
}

impl ReportManager {
    /// Print a single item, with a specific indent level.
    pub fn print_single_item(&self, item: &Item, indent: usize) {
        // TODO: add named arguments
        eprintln!(
            "{}{} [{:>02}]{}{} {}",
            std::iter::repeat(' ')
                .take(self.spaces_per_indent * indent)
                .collect::<String>(),
            match item.state {
                ItemState::Todo => 'o',
                ItemState::Done => 'x',
                ItemState::Note => '-',
            },
            item.ref_id.unwrap_or(item.internal_id),
            if item.description.is_empty() {
                ""
            } else {
                " (D)"
            },
            match item.context() {
                Some(c) => format!(" @{}", c),
                None => String::new(),
            },
            item.name,
        );
    }

    /// Checks if a filter passes through an item and prints the item with a style and a specific indent level if it
    /// passed.
    pub fn print_item_styled<F>(&self, item: &Item, style: ReportStyle, indent: usize, filter: F)
    where
        F: Fn(&Item) -> bool + Copy,
    {
        let filter_result = filter(item);

        match style {
            ReportStyle::Shallow => {
                if filter_result {
                    self.print_single_item(item, indent);
                }
            }
            ReportStyle::Brief => {
                if filter_result {
                    self.print_single_item(item, indent);

                    if item.children.len() > 0 {
                        self.print_item_styled(
                            &item.children[0],
                            ReportStyle::Shallow,
                            indent + 1,
                            filter,
                        );
                    }

                    if item.children.len() > 1 {
                        eprintln!(
                            "{}  {} more...",
                            std::iter::repeat(' ')
                                .take(self.spaces_per_indent * indent)
                                .collect::<String>(),
                            item.children.len() - 1
                        );
                    }
                }
            }
            ReportStyle::Tree => {
                if filter_result {
                    self.print_single_item(item, indent);

                    for child in &item.children {
                        self.print_item_styled(&child, ReportStyle::Tree, indent + 1, filter);
                    }
                }
            }
        }
    }

    /// Displays a collection of items with a specific report style and a filter.
    pub fn display_report<F>(&self, name: &str, report_list: &[&Item], style: ReportStyle, filter: F)
    where
        F: Fn(&Item) -> bool + Copy,
    {
        eprintln!("{} | {} selected items", name, report_list.len());

        for item in report_list {
            self.print_item_styled(item, style, 0, filter);
        }
    }
}
