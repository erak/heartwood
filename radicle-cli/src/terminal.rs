pub mod args;
pub mod cob;
pub mod command;
pub mod format;
pub mod io;
pub mod patch;
pub mod spinner;
pub mod table;
pub mod textbox;

use std::ffi::OsString;
use std::process;

use radicle::profile::Profile;

pub use args::{Args, Error, Help};
pub use console::measure_text_width as text_width;
pub use inquire::{ui::Styled, Editor};
pub use io::*;
pub use spinner::{spinner, Spinner};
pub use table::Table;
pub use textbox::TextBox;

mod styling {
    use std::collections::HashSet;
    use std::fmt::Display;

    use termion::color;
    use termion::style;

    #[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
    pub enum Style {
        White,
        Red,
        Green,
        Cyan,
        Blue,
        Magenta,
        Yellow,
        Bold,
        Underline,
        Italic,
        Reverse,
        Dim,
        BrightRed,
        BrightBlue,
        BrightGreen,
        BrightWhite,
    }

    impl Display for Style {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            use termion::color::Fg;

            match self {
                Self::Red => write!(f, "{}", Fg(color::Red)),
                Self::Green => write!(f, "{}", Fg(color::Green)),
                Self::BrightGreen => write!(f, "{}", Fg(color::LightGreen)),
                Self::Dim => write!(f, "{}", style::Faint),
                Self::Bold => write!(f, "{}", style::Bold),
                Self::Yellow => write!(f, "{}", Fg(color::Yellow)),
                Self::Blue => write!(f, "{}", Fg(color::Blue)),
                Self::Cyan => write!(f, "{}", Fg(color::Cyan)),
                Self::BrightBlue => write!(f, "{}", Fg(color::LightBlue)),
                Self::Italic => write!(f, "{}", style::Italic),
                x => todo!("{:?}", x),
            }
        }
    }

    pub struct Token<T> {
        content: T,
        styles: HashSet<Style>,
    }

    impl<T: Display> Display for Token<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let mut reset_style = false;
            let mut reset_color = false;

            for style in &self.styles {
                if matches!(style, Style::Italic | Style::Bold | Style::Dim) {
                    reset_style = true;
                } else {
                    reset_color = true;
                }
                write!(f, "{style}")?;
            }
            write!(f, "{}", self.content)?;

            if reset_style {
                write!(f, "{}", style::Reset)?;
            }
            if reset_color {
                write!(f, "{}", color::Fg(color::Reset))?;
            }
            Ok(())
        }
    }

    impl<T> Token<T> {
        pub fn red(&mut self) -> &mut Self {
            self.styles.insert(Style::Red);
            self
        }

        pub fn yellow(&mut self) -> &mut Self {
            self.styles.insert(Style::Yellow);
            self
        }

        pub fn bold(&mut self) -> &mut Self {
            self.styles.insert(Style::Bold);
            self
        }

        pub fn underline(&mut self) -> &mut Self {
            self.styles.insert(Style::Underline);
            self
        }

        pub fn reverse(&mut self) -> &mut Self {
            self.styles.insert(Style::Reverse);
            self
        }

        pub fn italic(&mut self) -> &mut Self {
            self.styles.insert(Style::Italic);
            self
        }

        pub fn white(&mut self) -> &mut Self {
            self.styles.insert(Style::White);
            self
        }

        pub fn dim(&mut self) -> &mut Self {
            self.styles.insert(Style::Dim);
            self
        }

        pub fn blue(&mut self) -> &mut Self {
            self.styles.insert(Style::Blue);
            self
        }

        pub fn cyan(&mut self) -> &mut Self {
            self.styles.insert(Style::Cyan);
            self
        }

        pub fn green(&mut self) -> &mut Self {
            self.styles.insert(Style::Green);
            self
        }

        pub fn bright_green(&mut self) -> &mut Self {
            self.styles.insert(Style::BrightGreen);
            self
        }

        pub fn bright_blue(&mut self) -> &mut Self {
            self.styles.insert(Style::BrightBlue);
            self
        }

        pub fn bright_red(&mut self) -> &mut Self {
            self.styles.insert(Style::BrightRed);
            self
        }

        pub fn bright_white(&mut self) -> &mut Self {
            self.styles.insert(Style::BrightWhite);
            self
        }

        pub fn magenta(&mut self) -> &mut Self {
            self.styles.insert(Style::Magenta);
            self
        }
    }

    pub fn style<T: Display>(content: T) -> Token<T> {
        Token {
            content,
            styles: HashSet::new(),
        }
    }
}
use styling::*;

/// Context passed to all commands.
pub trait Context {
    /// Return the currently active profile, or an error if no profile is active.
    fn profile(&self) -> Result<Profile, anyhow::Error>;
}

impl Context for Profile {
    fn profile(&self) -> Result<Profile, anyhow::Error> {
        Ok(self.clone())
    }
}

impl<F> Context for F
where
    F: Fn() -> Result<Profile, anyhow::Error>,
{
    fn profile(&self) -> Result<Profile, anyhow::Error> {
        self()
    }
}

/// A command that can be run.
pub trait Command<A: Args, C: Context> {
    /// Run the command, given arguments and a context.
    fn run(self, args: A, context: C) -> anyhow::Result<()>;
}

impl<F, A: Args, C: Context> Command<A, C> for F
where
    F: FnOnce(A, C) -> anyhow::Result<()>,
{
    fn run(self, args: A, context: C) -> anyhow::Result<()> {
        self(args, context)
    }
}

pub fn run_command<A, C>(help: Help, action: &str, cmd: C) -> !
where
    A: Args,
    C: Command<A, fn() -> anyhow::Result<Profile>>,
{
    let args = std::env::args_os().into_iter().skip(1).collect();

    run_command_args(help, action, cmd, args)
}

pub fn run_command_args<A, C>(help: Help, action: &str, cmd: C, args: Vec<OsString>) -> !
where
    A: Args,
    C: Command<A, fn() -> anyhow::Result<Profile>>,
{
    use io as term;

    let options = match A::from_args(args) {
        Ok((opts, unparsed)) => {
            if let Err(err) = args::finish(unparsed) {
                term::error(err);
                process::exit(1);
            }
            opts
        }
        Err(err) => {
            match err.downcast_ref::<Error>() {
                Some(Error::Help) => {
                    term::help(help.name, help.version, help.description, help.usage);
                    process::exit(0);
                }
                Some(Error::Usage) => {
                    term::usage(help.name, help.usage);
                    process::exit(1);
                }
                _ => {}
            };
            eprintln!(
                "{} {} {} {}",
                style("==").red(),
                style("Error:").red(),
                style(format!("rad-{}:", help.name)).red(),
                style(err.to_string()).red(),
            );

            if let Some(Error::WithHint { hint, .. }) = err.downcast_ref::<Error>() {
                eprintln!("{}", style(hint).yellow());
            }

            process::exit(1);
        }
    };

    match cmd.run(options, self::profile) {
        Ok(()) => process::exit(0),
        Err(err) => {
            term::fail(&format!("{action} failed"), &err);
            process::exit(1);
        }
    }
}

/// Get the default profile. Fails if there is no profile.
pub fn profile() -> Result<Profile, anyhow::Error> {
    let error = args::Error::WithHint {
        err: anyhow::anyhow!("Could not load radicle profile"),
        hint: "To setup your radicle profile, run `rad auth`.",
    };

    match Profile::load() {
        Ok(profile) => Ok(profile),
        Err(_) => Err(error.into()),
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Interactive {
    Yes,
    No,
}

impl Default for Interactive {
    fn default() -> Self {
        Interactive::No
    }
}

impl Interactive {
    pub fn yes(&self) -> bool {
        (*self).into()
    }

    pub fn no(&self) -> bool {
        !self.yes()
    }
}

impl From<Interactive> for bool {
    fn from(c: Interactive) -> Self {
        match c {
            Interactive::Yes => true,
            Interactive::No => false,
        }
    }
}

impl From<bool> for Interactive {
    fn from(b: bool) -> Self {
        if b {
            Interactive::Yes
        } else {
            Interactive::No
        }
    }
}
