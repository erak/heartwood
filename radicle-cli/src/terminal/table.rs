//! Print column-aligned text to the console.
//!
//! Example:
//! ```
//! use radicle_cli::terminal::table::*;
//!
//! let mut t = Table::new(TableOptions::default());
//! t.push(["pest", "biological control"]);
//! t.push(["aphid", "lacewing"]);
//! t.push(["spider mite", "ladybug"]);
//! t.render();
//! // pest        biological control
//! // aphid       ladybug
//! // spider mite persimilis
//! ```

use std::fmt::{Display, Write};

use crate::terminal as term;
use unicode_width::UnicodeWidthStr;

use super::Paint;

#[derive(Debug, Default)]
pub struct TableOptions {
    pub overflow: bool,
}

pub trait Cell: Display {
    type Truncated: Cell;
    type Padded: Cell;

    fn width(&self) -> usize;
    fn truncate(&self, width: usize, delim: &str) -> Self::Truncated;
    fn pad_left(&self, padding: usize) -> Self::Padded;
}

impl Cell for Paint<String> {
    type Truncated = Self;
    type Padded = Self;

    fn width(&self) -> usize {
        UnicodeWidthStr::width(self.content())
    }

    fn truncate(&self, width: usize, delim: &str) -> Self {
        let mut item = self.item.clone();
        // FIXME: This is not correct when dealing with non-ASCII characters.
        // We need to account for the fact that we may not be at a char boundary.
        String::truncate(&mut item, width);

        Self {
            item,
            style: self.style,
        }
    }

    fn pad_left(&self, padding: usize) -> Self {
        Self {
            item: format!("{self:padding$}"),
            style: self.style,
        }
    }
}

impl Cell for Paint<&str> {
    type Truncated = Self;
    type Padded = Paint<String>;

    fn width(&self) -> usize {
        UnicodeWidthStr::width(self.content())
    }

    fn truncate(&self, width: usize, delim: &str) -> Self {
        Self {
            // FIXME: This is not correct when dealing with non-ASCII characters.
            // We need to account for the fact that we may not be at a char boundary.
            item: &self.item[..width],
            style: self.style,
        }
    }

    fn pad_left(&self, padding: usize) -> Paint<String> {
        Paint {
            item: format!("{self:padding$}"),
            style: self.style,
        }
    }
}

impl Cell for String {
    type Truncated = Self;
    type Padded = Self;

    fn width(&self) -> usize {
        UnicodeWidthStr::width(self.as_str())
    }

    fn truncate(&self, width: usize, delim: &str) -> Self {
        let mut s = self.clone();
        String::truncate(&mut s, width);
        s
    }

    fn pad_left(&self, padding: usize) -> Self {
        format!("{self:padding$}")
    }
}

impl Cell for str {
    type Truncated = String;
    type Padded = String;

    fn width(&self) -> usize {
        UnicodeWidthStr::width(self)
    }

    fn truncate(&self, width: usize, delim: &str) -> String {
        self[..width].to_owned()
    }

    fn pad_left(&self, padding: usize) -> String {
        format!("{self:padding$}")
    }
}

#[derive(Debug)]
pub struct Table<const W: usize> {
    rows: Vec<[String; W]>,
    widths: [usize; W],
    opts: TableOptions,
}

impl<const W: usize> Default for Table<W> {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            widths: [0; W],
            opts: TableOptions::default(),
        }
    }
}

impl<const W: usize> Table<W> {
    pub fn new(opts: TableOptions) -> Self {
        Self {
            rows: Vec::new(),
            widths: [0; W],
            opts,
        }
    }

    pub fn push(&mut self, row: [impl Cell; W]) {
        let row = row.map(|s| s.to_string());
        for (i, cell) in row.iter().enumerate() {
            // match cell.down{}
            self.widths[i] = self.widths[i].max(cell.width());
        }
        self.rows.push(row);
    }

    pub fn render(self) {
        let width = term::width(); // Terminal width.

        for row in &self.rows {
            let mut output = String::new();
            let cells = row.len();

            for (i, cell) in row.iter().enumerate() {
                if i == cells - 1 || self.opts.overflow {
                    write!(output, "{cell}").ok();
                } else {
                    write!(output, "{} ", cell.pad_left(self.widths[i]),).ok();
                }
            }

            let output = output.trim_end();
            println!(
                "{}",
                if let Some(width) = width {
                    output.truncate(width - 1, "…")
                } else {
                    output.into()
                }
            );
        }
    }

    pub fn render_tree(self) {
        for (r, row) in self.rows.iter().enumerate() {
            if r != self.rows.len() - 1 {
                print!("├── ");
            } else {
                print!("└── ");
            }
            for (i, cell) in row.iter().enumerate() {
                print!("{} ", cell.pad_left(self.widths[i]));
            }
            println!();
        }
    }
}
