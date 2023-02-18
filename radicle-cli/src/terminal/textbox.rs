use std::fmt;

use crate::terminal as term;
use crate::terminal::cell::Cell as _;

pub struct TextBox {
    pub body: String,
    first: bool,
    last: bool,
}

impl TextBox {
    pub fn new(body: String) -> Self {
        Self {
            body,
            first: true,
            last: true,
        }
    }

    /// Is this text box the last one in the list?
    pub fn last(mut self, connect: bool) -> Self {
        self.last = connect;
        self
    }

    /// Is this text box the first one in the list?
    pub fn first(mut self, connect: bool) -> Self {
        self.first = connect;
        self
    }
}

impl fmt::Display for TextBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut width = self.body.lines().map(|l| l.width()).max().unwrap_or(0) + 2;
        if let Some(max) = term::columns() {
            if width + 2 > max {
                width = max - 2
            }
        }

        let (connector, header_width) = if !self.first {
            ("┴", width - 1)
        } else {
            ("", width)
        };
        writeln!(f, "┌{}{}┐", connector, "─".repeat(header_width))?;

        for l in self.body.lines() {
            writeln!(f, "│ {}│", l.pad(width - 1))?;
        }

        let (connector, footer_width) = if !self.last {
            ("┬", width - 1)
        } else {
            ("", width)
        };

        writeln!(f, "└{}{}┘", connector, "─".repeat(footer_width))?;

        if !self.last {
            writeln!(f, " │")?;
        }
        Ok(())
    }
}
