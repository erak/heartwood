use std::hash::Hash;
use std::time::Duration;

use anyhow::Result;

use tuirealm::terminal::TerminalBridge;
use tuirealm::Frame;
use tuirealm::{Application, EventListenerCfg, NoUserEvent};

pub mod components;
pub mod layout;

/// Trait that must be implemented by client applications in order to be run
/// as tui-application using tui-realm. Implementors act as models to the
/// tui-realm application that can be polled for new messages, updated
/// accordingly and rendered with new state.
///
/// Please see `examples/` for further information on how to use it.
pub trait Tui<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    /// Should initialize an application by mounting and activating components.
    fn init(&mut self, app: &mut Application<Id, Message, NoUserEvent>) -> Result<()>;

    /// Should update the current state by handling a message from the view.
    fn update(&mut self, app: &mut Application<Id, Message, NoUserEvent>);

    /// Should draw the application to a frame.
    fn view(&mut self, app: &mut Application<Id, Message, NoUserEvent>, frame: &mut Frame);

    /// Should return true if the application is requested to quit.
    fn quit(&self) -> bool;
}

/// A tui-window using the cross-platform Terminal helper provided
/// by tui-realm.
pub struct Window {
    /// Helper around `Terminal` to quickly setup and perform on terminal.
    pub terminal: TerminalBridge,
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

/// Provides a way to create and run a new tui-application.
impl Window {
    /// Creates a tui-window using the default cross-platform Terminal
    /// helper and panics if its creation fails.
    pub fn new() -> Self {
        Self {
            terminal: TerminalBridge::new().expect("Cannot create terminal bridge"),
        }
    }

    /// Runs this tui-window with the tui-application given and performs the
    /// following steps:
    /// 1. Enter alternative terminal screen
    /// 2. Run main loop until application should quit and with each iteration
    ///    - poll new events (tick or user event)
    ///    - update application state
    ///    - redraw view
    /// 3. Leave alternative terminal screen
    pub fn run<T, Id, Message>(&mut self, tui: &mut T) -> Result<()>
    where
        T: Tui<Id, Message>,
        Id: Eq + PartialEq + Clone + Hash,
        Message: Eq,
    {
        let _ = self.terminal.enable_raw_mode();
        let _ = self.terminal.enter_alternate_screen();
        let mut app = Application::init(
            EventListenerCfg::default().default_input_listener(Duration::from_millis(10)),
        );

        tui.init(&mut app)?;

        while !tui.quit() {
            tui.update(&mut app);

            self.terminal.raw_mut().draw(|frame| {
                tui.view(&mut app, frame);
            })?;
        }

        let _ = self.terminal.leave_alternate_screen();
        let _ = self.terminal.disable_raw_mode();

        Ok(())
    }
}
