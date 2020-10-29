use crate::item::{Item, State};

#[derive(Clone, Copy)]
pub enum ReportStyle {
    Shallow,
    Brief,
    Tree,
}

pub struct ReportManager {
    pub spaces_per_indent: usize,
}

impl ReportManager {
    pub fn print_single_item(&self, item: &Item, indent: usize) {
        eprintln!(
            "{}{} [{:>02}]{} {}",
            std::iter::repeat(' ')
                .take(self.spaces_per_indent * indent)
                .collect::<String>(),
            match item.state {
                State::Todo => 'o',
                State::Done => 'x',
                State::Note => '-',
            },
            item.ref_id.unwrap_or(item.internal_id),
            match item.context() {
                Some(c) => format!(" @{}", c),
                None => String::new(),
            },
            item.name,
        );
    }

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

    // TODO: add sort methods
    pub fn display_report<F>(&self, name: &str, report_list: &[&Item], style: ReportStyle, f: F)
    where
        F: Fn(&Item) -> bool + Copy,
    {
        eprintln!("{} | {} selected items", name, report_list.len());

        for item in report_list {
            self.print_item_styled(item, style, 0, f);
        }
    }
}
