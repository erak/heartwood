use tui_realm_stdlib::{Phantom, Textarea};

use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::{Component, MockComponent, NoUserEvent};

use crate::tui::components::{ApplicationTitle, ShortcutBar};

use super::app::Message;

/// Since `terminal-tui` does not know the type of messages that are being
/// passed around in the app, the following handlers need to be implemented for
/// each component used.
impl Component<Message, NoUserEvent> for ApplicationTitle {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl Component<Message, NoUserEvent> for Textarea {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl Component<Message, NoUserEvent> for ShortcutBar {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

/// Some user events need to be handled globally (e.g. user presses key `q` to quit
/// the application). This component can be used in conjunction with SubEventClause
/// to handle those events.
#[derive(Default, MockComponent)]
pub struct GlobalPhantom {
    component: Phantom,
}

impl Component<Message, NoUserEvent> for GlobalPhantom {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Message::Quit),
            _ => None,
        }
    }
}
