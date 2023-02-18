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
//! ```
//! Output:
//! ``` plain
//! pest        biological control
//! aphid       ladybug
//! spider mite persimilis
//! ```

use std::fmt::Display;
use std::io;

use crate::terminal as term;
use unicode_width::UnicodeWidthStr;

use super::Paint;

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct Max {
    width: Option<usize>,
    height: Option<usize>,
}

#[derive(Debug, Default)]
pub struct TableOptions {
    pub overflow: bool,
    pub max: Max,
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
        Self {
            item: self.item.truncate(width, delim),
            style: self.style,
        }
    }

    fn pad_left(&self, padding: usize) -> Self {
        Self {
            item: self.item.pad_left(padding),
            style: self.style,
        }
    }
}

impl Cell for Paint<&str> {
    type Truncated = Paint<String>;
    type Padded = Paint<String>;

    fn width(&self) -> usize {
        Cell::width(self.item)
    }

    fn truncate(&self, width: usize, delim: &str) -> Paint<String> {
        Paint {
            item: self.item.truncate(width, delim),
            style: self.style,
        }
    }

    fn pad_left(&self, padding: usize) -> Paint<String> {
        Paint {
            item: self.item.pad_left(padding),
            style: self.style,
        }
    }
}

impl Cell for String {
    type Truncated = Self;
    type Padded = Self;

    fn width(&self) -> usize {
        Cell::width(self.as_str())
    }

    fn truncate(&self, width: usize, delim: &str) -> Self {
        self.as_str().truncate(width, delim)
    }

    fn pad_left(&self, padding: usize) -> Self {
        self.as_str().pad_left(padding)
    }
}

impl Cell for str {
    type Truncated = String;
    type Padded = String;

    fn width(&self) -> usize {
        UnicodeWidthStr::width(self)
    }

    fn truncate(&self, width: usize, delim: &str) -> String {
        use unicode_segmentation::UnicodeSegmentation as _;

        if width < Cell::width(self) {
            let d = Cell::width(delim);
            if width < d {
                // If we can't even fit the delimiter, just return an empty string.
                return String::new();
            }
            // Find the unicode byte boundary where the display width is the largest,
            // while being smaller than the given max width.
            let mut cols = 0;
            let mut boundary = 0;
            for g in self.graphemes(true) {
                let c = Cell::width(g);
                if cols + c + d > width {
                    break;
                }
                boundary += g.len();
                cols += c;
            }
            format!("{}{delim}", &self[..boundary])
        } else {
            self.to_owned()
        }
    }

    fn pad_left(&self, max: usize) -> String {
        let width = Cell::width(self);

        if width < max {
            format!("{self}{}", " ".repeat(max - width))
        } else {
            self.to_owned()
        }
    }
}

impl<T: Cell + ?Sized> Cell for &T {
    type Truncated = T::Truncated;
    type Padded = T::Padded;

    fn width(&self) -> usize {
        T::width(self)
    }

    fn truncate(&self, width: usize, delim: &str) -> Self::Truncated {
        T::truncate(self, width, delim)
    }

    fn pad_left(&self, padding: usize) -> Self::Padded {
        T::pad_left(self, padding)
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
            self.widths[i] = self.widths[i].max(cell.width());
        }
        self.rows.push(row);
    }

    pub fn render(self) {
        self.write(io::stdout()).ok();
    }

    pub fn write<T: io::Write>(self, mut writer: T) -> io::Result<()> {
        let width = self.opts.max.width.or_else(term::columns);

        for row in &self.rows {
            let mut output = String::new();
            let cells = row.len();

            for (i, cell) in row.iter().enumerate() {
                if i == cells - 1 || self.opts.overflow {
                    output.push_str(cell.to_string().as_str());
                } else {
                    output.push_str(cell.pad_left(self.widths[i]).as_str());
                    output.push(' ');
                }
            }

            let output = output.trim_end();
            writeln!(
                writer,
                "{}",
                if let Some(width) = width {
                    output.truncate(width, "…")
                } else {
                    output.into()
                }
            )?;
        }
        Ok(())
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

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_truncate() {
        assert_eq!("🍍".truncate(1, "…"), String::from("…"));
        assert_eq!("🍍".truncate(1, ""), String::from(""));
        assert_eq!("🍍🍍".truncate(2, "…"), String::from("…"));
        assert_eq!("🍍🍍".truncate(3, "…"), String::from("🍍…"));
        assert_eq!("🍍".truncate(1, "🍎"), String::from(""));
        assert_eq!("🍍".truncate(2, "🍎"), String::from("🍍"));
        assert_eq!("🍍🍍".truncate(3, "🍎"), String::from("🍎"));
        assert_eq!("🍍🍍🍍".truncate(4, "🍎"), String::from("🍍🍎"));
        assert_eq!("hello".truncate(3, "…"), String::from("he…"));
    }

    #[test]
    fn test_table() {
        let mut s = Vec::new();
        let mut t = Table::new(TableOptions::default());

        t.push(["pineapple", "rosemary"]);
        t.push(["apples", "pears"]);
        t.write(&mut s).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            String::from_utf8_lossy(&s),
            [
                "pineapple rosemary\n",
                "apples    pears\n"
            ].join("")
        );
    }

    #[test]
    fn test_table_truncate() {
        let mut s = Vec::new();
        let mut t = Table::new(TableOptions {
            max: Max {
                width: Some(16),
                height: None,
            },
            ..TableOptions::default()
        });

        t.push(["pineapple", "rosemary"]);
        t.push(["apples", "pears"]);
        t.write(&mut s).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            String::from_utf8_lossy(&s),
            [
                "pineapple rosem…\n",
                "apples    pears\n"
            ].join("")
        );
    }

    #[test]
    fn test_table_unicode() {
        let mut s = Vec::new();
        let mut t = Table::new(TableOptions::default());

        t.push(["🍍pineapple", "__rosemary", "__sage"]);
        t.push(["__pears", "🍎apples", "🍌bananas"]);
        t.write(&mut s).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            String::from_utf8_lossy(&s),
            [
                "🍍pineapple __rosemary __sage\n",
                "__pears     🍎apples   🍌bananas\n"
            ].join("")
        );
    }

    #[test]
    fn test_table_unicode_truncate() {
        let mut s = Vec::new();
        let mut t = Table::new(TableOptions {
            max: Max {
                width: Some(16),
                height: None,
            },
            ..TableOptions::default()
        });

        t.push(["🍍pineapple", "__rosemary"]);
        t.push(["__pears", "🍎apples"]);
        t.write(&mut s).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            String::from_utf8_lossy(&s),
            [
                "🍍pineapple __r…\n",
                "__pears     🍎a…\n"
            ].join("")
        );
    }
}
