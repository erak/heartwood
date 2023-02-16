use std::fmt;
use std::str::FromStr;

use inquire::ui::{ErrorMessageRenderConfig, StyleSheet, Styled};
use inquire::{ui::Color, ui::RenderConfig, Confirm, CustomType, Password, Select, Text};

use radicle::cob::issue::Issue;
use radicle::cob::thread::CommentId;
use radicle::crypto::ssh::keystore::Passphrase;
use radicle::crypto::Signer;
use radicle::profile;
use radicle::profile::Profile;

use radicle_crypto::ssh::keystore::MemorySigner;

use super::command;
use super::format;
use super::spinner::spinner;
use super::style;
use super::Error;

pub const TAB: &str = "    ";

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => ({
        println!("{}", format_args!($($arg)*));
    })
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => ({
        $crate::terminal::io::success_args(format_args!($($arg)*));
    })
}

#[macro_export]
macro_rules! tip {
    ($($arg:tt)*) => ({
        $crate::terminal::io::tip_args(format_args!($($arg)*));
    })
}

pub use info;
pub use success;
pub use tip;

pub fn success_args(args: fmt::Arguments) {
    println!("{} {args}", style("✓").green());
}

pub fn tip_args(args: fmt::Arguments) {
    println!("👉 {}", style(format!("{args}")).italic());
}

pub fn width() -> Option<usize> {
    console::Term::stdout()
        .size_checked()
        .map(|(_, cols)| cols as usize)
}

pub fn headline(headline: &str) {
    println!();
    println!("{}", style(headline).bold());
    println!();
}

pub fn header(header: &str) {
    println!();
    println!("{}", style(format::yellow(header)).bold().underline());
    println!();
}

pub fn blob(text: impl fmt::Display) {
    println!("{}", style(text.to_string().trim()).dim());
}

pub fn blank() {
    println!()
}

pub fn print(msg: impl fmt::Display) {
    println!("{msg}");
}

pub fn prefixed(prefix: &str, text: &str) -> String {
    text.split('\n')
        .map(|line| format!("{prefix}{line}\n"))
        .collect()
}

pub fn help(name: &str, version: &str, description: &str, usage: &str) {
    println!("rad-{name} {version}\n{description}\n{usage}");
}

pub fn usage(name: &str, usage: &str) {
    println!(
        "{} {}\n{}",
        style("×").red(),
        style(format!("Error: rad-{name}: invalid usage")).red(),
        style(prefixed(TAB, usage)).red().dim()
    );
}

pub fn println(prefix: impl fmt::Display, msg: impl fmt::Display) {
    println!("{prefix} {msg}");
}

pub fn indented(msg: impl fmt::Display) {
    println!("{TAB}{msg}");
}

pub fn subcommand(msg: impl fmt::Display) {
    println!("{} {}", style("$").dim(), style(msg).dim());
}

pub fn warning(warning: &str) {
    println!(
        "{} {} {warning}",
        style("!").yellow(),
        style("Warning:").yellow().bold(),
    );
}

pub fn error(error: impl fmt::Display) {
    println!("{} {error}", style("×").red());
}

pub fn fail(header: &str, error: &anyhow::Error) {
    let err = error.to_string();
    let err = err.trim_end();
    let separator = if err.contains('\n') { ":\n" } else { ": " };

    println!(
        "{} {}{}{error}",
        style("×").red(),
        style(header).red().bold(),
        style(separator).red(),
    );

    if let Some(Error::WithHint { hint, .. }) = error.downcast_ref::<Error>() {
        println!("{} {}", style("×").yellow(), style(hint).yellow());
        blank();
    }
}

pub fn ask<D: fmt::Display>(prompt: D, default: bool) -> bool {
    let prompt = format!("{} {}", style("☞".to_owned()).white(), prompt);

    Confirm::new(&prompt)
        .with_default(true)
        .prompt()
        .unwrap_or_default()
}

pub fn confirm<D: fmt::Display>(prompt: D) -> bool {
    ask(format::tertiary(prompt), true)
}

pub fn abort<D: fmt::Display>(prompt: D) -> bool {
    ask(format::tertiary(prompt), false)
}

/// Get the signer. First we try getting it from ssh-agent, otherwise we prompt the user.
pub fn signer(profile: &Profile) -> anyhow::Result<Box<dyn Signer>> {
    if let Ok(signer) = profile.signer() {
        return Ok(signer);
    }

    let passphrase = secret_input();
    let spinner = spinner("Unsealing key...");
    let signer = MemorySigner::load(&profile.keystore, passphrase)?;

    spinner.finish();
    Ok(signer.boxed())
}

// pub fn theme() -> ColorfulTheme {
//     ColorfulTheme {
//         success_prefix: style("✓".to_owned()).for_stderr().green(),
//         prompt_prefix: style("☛".to_owned()).white().for_stderr(),
//         prompt_suffix: style("".to_owned()).cyan().for_stderr(),
//         prompt_style: Style::new().cyan().bold().for_stderr(),
//         active_item_style: Style::new().for_stderr().yellow().reverse(),
//         active_item_prefix: style("☛".to_owned()).yellow().for_stderr(),
//         picked_item_prefix: style("☛".to_owned()).yellow().for_stderr(),
//         inactive_item_prefix: style(" ".to_string()).for_stderr(),
//         inactive_item_style: Style::new().yellow().for_stderr(),
//         error_prefix: style("× Error:".to_owned()).red().for_stderr(),
//         success_suffix: style(":".to_owned()).cyan().for_stderr(),

//         ..ColorfulTheme::default()
//     }
// }

fn theme() -> RenderConfig {
    RenderConfig {
        prompt: StyleSheet::new().with_fg(Color::LightCyan),
        prompt_prefix: Styled::new("?").with_fg(Color::LightBlue),
        answered_prompt_prefix: Styled::new("✓").with_fg(Color::LightGreen),
        answer: StyleSheet::new(),
        error_message: ErrorMessageRenderConfig::default_colored()
            .with_prefix(Styled::new("×").with_fg(Color::LightRed)),
        ..RenderConfig::default_colored() // prompt_prefix: Styled<&'static str>,
                                          // answered_prompt_prefix: Styled<&'static str>,
                                          // prompt: StyleSheet,
                                          // default_value: StyleSheet,
                                          // placeholder: StyleSheet,
                                          // help_message: StyleSheet,
                                          // password_mask: char,
                                          // text_input: StyleSheet,
                                          // answer: StyleSheet,
                                          // canceled_prompt_indicator: Styled<&'static str>,
                                          // error_message: ErrorMessageRenderConfig,
                                          // highlighted_option_prefix: Styled<&'static str>,
                                          // scroll_up_prefix: Styled<&'static str>,
                                          // scroll_down_prefix: Styled<&'static str>,
                                          // selected_checkbox: Styled<&'static str>,
                                          // unselected_checkbox: Styled<&'static str>,
                                          // option_index_prefix: IndexPrefix,
                                          // option: StyleSheet,
                                          // calendar: CalendarRenderConfig,
                                          // editor_prompt: StyleSheet,
    }
}

pub fn text_input<S, E>(message: &str, default: Option<S>) -> anyhow::Result<S>
where
    S: fmt::Display + std::str::FromStr<Err = E> + Clone,
    E: fmt::Debug + fmt::Display,
{
    let theme = theme();
    let mut input = CustomType::<S>::new(message).with_render_config(theme);

    let value = match default {
        Some(default) => input.with_default(default).prompt()?,
        None => input.prompt()?,
    };
    Ok(value)
}

#[derive(Debug, Default, Clone)]
pub struct Optional<T> {
    option: Option<T>,
}

impl<T: fmt::Display> fmt::Display for Optional<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(val) = &self.option {
            write!(f, "{val}")
        } else {
            write!(f, "")
        }
    }
}

impl<T: FromStr> FromStr for Optional<T> {
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Optional { option: None });
        }
        let val: T = s.parse()?;

        Ok(Self { option: Some(val) })
    }
}

pub fn text_input_optional<S, E>(message: &str, initial: Option<S>) -> anyhow::Result<Option<S>>
where
    S: fmt::Display + fmt::Debug + FromStr<Err = E> + Clone,
    E: fmt::Debug + fmt::Display,
{
    let theme = theme();
    let input = CustomType::<Optional<S>>::new(message).with_render_config(theme);
    let value = if let Some(init) = initial {
        input.with_default(Optional { option: Some(init) }).prompt()
    } else {
        input.prompt()
    }?;

    Ok(value.option)
}

pub fn secret_input() -> Passphrase {
    secret_input_with_prompt("Passphrase:")
}

// TODO: This prompt shows success just for entering a password,
// even if the password is later found out to be wrong.
// We should handle this differently.
pub fn secret_input_with_prompt(prompt: &str) -> Passphrase {
    Passphrase::from(
        Password::new(prompt)
            .with_render_config(theme())
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .without_confirmation()
            .prompt()
            .unwrap(),
    )
}

pub fn secret_input_with_confirmation() -> Passphrase {
    Passphrase::from(
        Password::new("Passphrase:")
            .with_render_config(theme())
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .with_custom_confirmation_message("Repeat passphrase:")
            .with_custom_confirmation_error_message("The passphrases don't match.")
            .prompt()
            .unwrap(),
    )
}

pub fn secret_stdin() -> Result<Passphrase, anyhow::Error> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    Ok(Passphrase::from(input.trim_end().to_owned()))
}

pub fn read_passphrase(stdin: bool, confirm: bool) -> Result<Passphrase, anyhow::Error> {
    let passphrase = match profile::env::read_passphrase() {
        Some(input) => input,
        None => {
            if stdin {
                secret_stdin()?
            } else if confirm {
                secret_input_with_confirmation()
            } else {
                secret_input()
            }
        }
    };

    Ok(passphrase)
}

pub fn select<'a, T>(options: &'a [T], active: &'a T) -> Option<&'a T>
where
    T: fmt::Display + Eq + PartialEq + Clone,
{
    let theme = theme();
    let active = options.iter().position(|o| o == active);
    let selection = Select::new("", options.iter().collect::<Vec<_>>()).with_render_config(theme);

    let result = if let Some(active) = active {
        selection
            .with_starting_cursor(active)
            .prompt_skippable()
            .unwrap()
    } else {
        selection.prompt_skippable().unwrap()
    };

    result
}

pub fn select_with_prompt<'a, T>(prompt: &str, options: &'a [T], active: &'a T) -> Option<&'a T>
where
    T: fmt::Display + Eq + PartialEq,
{
    let theme = theme();
    let active = options.iter().position(|o| o == active);
    let selection =
        Select::new(prompt, options.iter().collect::<Vec<_>>()).with_render_config(theme);

    let result = if let Some(active) = active {
        selection.with_starting_cursor(active).prompt_skippable()
    } else {
        selection.prompt_skippable()
    }
    .unwrap();

    result
}

pub fn comment_select(issue: &Issue) -> Option<CommentId> {
    todo!();
    // let mut items = vec![issue.description().unwrap_or_default()];
    // items.extend(
    //     issue
    //         .comments()
    //         .map(|(, i)| i.body().to_owned())
    //         .collect::<Vec<_>>(),
    // );

    // let theme = theme();
    // let selection = Select::new("Which comment do you want to react to?", items)
    //     .with_render_config(theme)
    //     .with_starting_cursor(0)
    //     .prompt()
    //     .unwrap();

    // selection
    //     .and_then(|n| issue.comments().nth(n))
    //     .map(|(id, _)| *id)
}

pub fn markdown(content: &str) {
    if !content.is_empty() && command::bat(["-p", "-l", "md"], content).is_err() {
        blob(content);
    }
}

fn _info(args: std::fmt::Arguments) {
    println!("{args}");
}

pub mod proposal {
    use std::fmt::Write as _;

    use radicle::{
        cob::identity::{self, Proposal},
        git::Oid,
        identity::Identity,
    };

    use super::{super::format, theme};

    pub fn revision_select(
        proposal: &Proposal,
    ) -> Option<(&identity::RevisionId, &identity::Revision)> {
        todo!();
        // let selection = dialoguer::Select::with_theme(&theme())
        //     .with_prompt("Which revision do you want to select?")
        //     .items(
        //         &proposal
        //             .revisions()
        //             .map(|(rid, _)| rid.to_string())
        //             .collect::<Vec<_>>(),
        //     )
        //     .default(0)
        //     .interact_opt()
        //     .unwrap();

        // selection.and_then(|n| proposal.revisions().nth(n))
    }

    pub fn revision_commit_select<'a>(
        proposal: &'a Proposal,
        previous: &'a Identity<Oid>,
    ) -> Option<(&'a identity::RevisionId, &'a identity::Revision)> {
        todo!();
        // let selection = dialoguer::Select::with_theme(&theme())
        //     .with_prompt("Which revision do you want to commit?")
        //     .items(
        //         &proposal
        //             .revisions()
        //             .filter(|(_, r)| r.is_quorum_reached(previous))
        //             .map(|(rid, _)| rid.to_string())
        //             .collect::<Vec<_>>(),
        //     )
        //     .default(0)
        //     .interact_opt()
        //     .unwrap();

        // selection.and_then(|n| proposal.revisions().nth(n))
    }

    pub fn diff(proposal: &identity::Revision, previous: &Identity<Oid>) -> anyhow::Result<String> {
        use similar::{ChangeTag, TextDiff};

        let new = serde_json::to_string_pretty(&proposal.proposed)?;
        let previous = serde_json::to_string_pretty(&previous.doc)?;
        let diff = TextDiff::from_lines(&previous, &new);
        let mut buf = String::new();
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => write!(buf, "{}", format::negative(format!("-{change}")))?,
                ChangeTag::Insert => write!(buf, "{}", format::positive(format!("+{change}")))?,
                ChangeTag::Equal => write!(buf, " {change}")?,
            };
        }

        Ok(buf)
    }
}
