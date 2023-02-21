use std::ffi::OsString;

use crate::terminal as term;
use crate::terminal::args::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "tui",
    description: "Run TUI",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad tui [<option>...]

    Runs the terminal UI.

Options

    --help              Print help
"#,
};

pub struct Options;

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options {}, vec![]))
    }
}

pub fn run(_options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let _storage = &profile.storage;
    let _signer = term::signer(&profile)?;

    // Run TUI

    Ok(())
}
