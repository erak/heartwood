use std::ffi::OsString;

use anyhow::{anyhow, Context};

use crate::terminal as term;
use crate::terminal::args::{Args, Error, Help};

use crate::tui::Window;

use radicle::storage::ReadStorage;

#[path = "tui/app.rs"]
mod app;
#[path = "tui/components.rs"]
mod components;

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
    let (_, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = &profile.storage;

    let payload = storage
        .get(signer.public_key(), id)?
        .context("No project with such `id` exists")?;

    let project = payload.project()?;

    let mut window = Window::default();
    window.run(&mut app::App::new(id, project))?;

    Ok(())
}
