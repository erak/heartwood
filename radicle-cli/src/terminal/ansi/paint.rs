use std::fmt;

use unicode_width::UnicodeWidthStr;

use super::color::Color;
use super::style::{Property, Style};

/// A structure encapsulating an item and styling.
#[derive(Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Copy, Clone)]
pub struct Paint<T> {
    pub item: T,
    pub style: Style,
}

impl Paint<&str> {
    /// Return plain content.
    pub fn content(&self) -> &str {
        self.item
    }
}

impl Paint<String> {
    /// Return plain content.
    pub fn content(&self) -> &str {
        self.item.as_str()
    }
}

impl<T> Paint<T> {
    /// Constructs a new `Paint` structure encapsulating `item` with no set
    /// styling.
    #[inline]
    pub fn new(item: T) -> Paint<T> {
        Paint {
            item,
            style: Style::default(),
        }
    }

    /// Constructs a new _wrapping_ `Paint` structure encapsulating `item` with
    /// default styling.
    ///
    /// A wrapping `Paint` converts all color resets written out by the internal
    /// value to the styling of itself. This allows for seamless color wrapping
    /// of other colored text.
    ///
    /// # Performance
    ///
    /// In order to wrap an internal value, the internal value must first be
    /// written out to a local buffer and examined. As a result, displaying a
    /// wrapped value is likely to result in a heap allocation and copy.
    #[inline]
    pub fn wrapping(item: T) -> Paint<T> {
        Paint::new(item).wrap()
    }

    /// Constructs a new `Paint` structure encapsulating `item` with the
    /// foreground color set to the RGB color `r`, `g`, `b`.
    #[inline]
    pub fn rgb(r: u8, g: u8, b: u8, item: T) -> Paint<T> {
        Paint::new(item).fg(Color::RGB(r, g, b))
    }

    /// Constructs a new `Paint` structure encapsulating `item` with the
    /// foreground color set to the fixed 8-bit color `color`.
    #[inline]
    pub fn fixed(color: u8, item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Fixed(color))
    }

    pub fn red(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Red)
    }

    pub fn black(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Black)
    }

    pub fn yellow(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Yellow)
    }

    pub fn green(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Green)
    }

    pub fn cyan(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Cyan)
    }

    pub fn blue(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Blue)
    }

    pub fn magenta(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::Magenta)
    }

    pub fn white(item: T) -> Paint<T> {
        Paint::new(item).fg(Color::White)
    }

    /// Retrieves the style currently set on `self`.
    #[inline]
    pub fn style(&self) -> Style {
        self.style
    }

    /// Retrieves a borrow to the inner item.
    #[inline]
    pub fn inner(&self) -> &T {
        &self.item
    }

    /// Sets the style of `self` to `style`.
    #[inline]
    pub fn with_style(mut self, style: Style) -> Paint<T> {
        self.style = style;
        self
    }

    /// Makes `self` a _wrapping_ `Paint`.
    ///
    /// A wrapping `Paint` converts all color resets written out by the internal
    /// value to the styling of itself. This allows for seamless color wrapping
    /// of other colored text.
    ///
    /// # Performance
    ///
    /// In order to wrap an internal value, the internal value must first be
    /// written out to a local buffer and examined. As a result, displaying a
    /// wrapped value is likely to result in a heap allocation and copy.
    #[inline]
    pub fn wrap(mut self) -> Paint<T> {
        self.style.wrap = true;
        self
    }

    /// Sets the foreground to `color`.
    #[inline]
    pub fn fg(mut self, color: Color) -> Paint<T> {
        self.style.foreground = color;
        self
    }

    /// Sets the background to `color`.
    #[inline]
    pub fn bg(mut self, color: Color) -> Paint<T> {
        self.style.background = color;
        self
    }

    pub fn bold(mut self) -> Self {
        self.style.properties.set(Property::BOLD);
        self
    }

    pub fn dim(mut self) -> Self {
        self.style.properties.set(Property::DIM);
        self
    }

    pub fn italic(mut self) -> Self {
        self.style.properties.set(Property::ITALIC);
        self
    }

    pub fn underline(mut self) -> Self {
        self.style.properties.set(Property::UNDERLINE);
        self
    }

    pub fn invert(mut self) -> Self {
        self.style.properties.set(Property::INVERT);
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.style.properties.set(Property::STRIKETHROUGH);
        self
    }

    pub fn blink(mut self) -> Self {
        self.style.properties.set(Property::BLINK);
        self
    }

    pub fn hidden(mut self) -> Self {
        self.style.properties.set(Property::HIDDEN);
        self
    }
}

impl<T: UnicodeWidthStr> UnicodeWidthStr for Paint<T> {
    fn width(&self) -> usize {
        self.item.width()
    }

    fn width_cjk(&self) -> usize {
        self.item.width_cjk()
    }
}

impl<T: fmt::Display> fmt::Display for Paint<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if Paint::is_enabled() && self.style.wrap {
            let mut prefix = String::new();
            prefix.push_str("\x1B[0m");
            self.style.fmt_prefix(&mut prefix)?;
            self.style.fmt_prefix(f)?;

            let item = format!("{}", self.item).replace("\x1B[0m", &prefix);
            fmt::Display::fmt(&item, f)?;
            self.style.fmt_suffix(f)
        } else if Paint::is_enabled() {
            self.style.fmt_prefix(f)?;
            fmt::Display::fmt(&self.item, f)?;
            self.style.fmt_suffix(f)
        } else {
            fmt::Display::fmt(&self.item, f)
        }
    }
}

impl Paint<()> {
    /// Returns `true` if coloring is enabled and `false` otherwise.
    pub fn is_enabled() -> bool {
        concolor::get(concolor::Stream::Stdout).ansi_color()
    }
}

/// Shorthand for [`Paint::new`].
pub fn paint<T>(item: T) -> Paint<T> {
    Paint::new(item)
}
