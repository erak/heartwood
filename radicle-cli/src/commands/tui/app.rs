use anyhow::Result;

use tui_realm_stdlib::Textarea;

use tuirealm::application::PollStrategy;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, BorderSides, Borders, Color, TextSpan};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::{Application, Frame, NoUserEvent, Sub, SubClause, SubEventClause};

use crate::tui::components::{ApplicationTitle, Property, PropertyList, Shortcut, ShortcutBar};
use crate::tui::Tui;

use super::components::GlobalPhantom;

use radicle::identity::{Id, Project};

pub struct App {
    id: Id,
    project: Project,
    quit: bool,
}

/// Messages handled by this application.
#[derive(Debug, Eq, PartialEq)]
pub enum Message {
    Quit,
}

/// All components known to this application.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Component {
    Title,
    Content,
    Shortcuts,
    GlobalPhantom,
}

/// Creates a new application using a tui-realm-application, mounts all
/// components and sets focus to a default one.
impl App {
    pub fn status(&self) -> Vec<TextSpan> {
        vec![
            TextSpan::new("Project").fg(Color::Gray),
            TextSpan::new(String::new()),
            TextSpan::new(format!("{}", self.id)),
        ]
    }

    fn layout(
        app: &mut Application<Component, Message, NoUserEvent>,
        frame: &mut Frame,
    ) -> Vec<Rect> {
        let area = frame.size();
        let title_h = app
            .query(&Component::Title, Attribute::Height)
            .ok()
            .flatten()
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let shortcuts_h = app
            .query(&Component::Shortcuts, Attribute::Height)
            .ok()
            .flatten()
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let container_h = area.height.saturating_sub(title_h + shortcuts_h);

        Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(title_h),
                    Constraint::Length(container_h - 2),
                    Constraint::Length(shortcuts_h),
                ]
                .as_ref(),
            )
            .split(area)
    }
}

impl App {
    pub fn new(id: Id, project: Project) -> Self {
        Self {
            id,
            project,
            quit: false,
        }
    }
}

impl Tui<Component, Message> for App {
    fn init(&mut self, app: &mut Application<Component, Message, NoUserEvent>) -> Result<()> {
        app.mount(
            Component::Title,
            Box::new(ApplicationTitle::new(&format!("{}", self.project.name()))),
            vec![],
        )?;
        app.mount(
            Component::Content,
            Box::new(
                PropertyList::default()
                    .child(Property::new("Id", &format!("{}", self.id)))
                    .child(Property::new("Name", &format!("{}", self.project.name())))
                    .child(Property::new(
                        "Description",
                        &format!("{}", self.project.description()),
                    )),
            ),
            vec![],
        )?;
        app.mount(
            Component::Shortcuts,
            Box::new(
                ShortcutBar::default()
                    .child(Shortcut::new("s", "status"))
                    .child(Shortcut::new("q", "quit")),
            ),
            vec![],
        )?;

        // Add global key listener and subscribe to key events
        app.mount(
            Component::GlobalPhantom,
            Box::new(GlobalPhantom::default()),
            vec![Sub::new(
                SubEventClause::Keyboard(KeyEvent {
                    code: Key::Char('q'),
                    modifiers: KeyModifiers::NONE,
                }),
                SubClause::Always,
            )],
        )?;

        // We need to give focus to a component then
        app.active(&Component::Title)?;

        Ok(())
    }

    fn view(&mut self, app: &mut Application<Component, Message, NoUserEvent>, frame: &mut Frame) {
        let layout = Self::layout(app, frame);

        app.view(&Component::Title, frame, layout[0]);
        app.view(&Component::Content, frame, layout[1]);
        app.view(&Component::Shortcuts, frame, layout[2]);
    }

    fn update(&mut self, app: &mut Application<Component, Message, NoUserEvent>) {
        match app.tick(PollStrategy::Once) {
            Ok(messages) => {
                for message in messages {
                    match message {
                        Message::Quit => self.quit = true,
                    }
                }
            }
            _ => {}
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }
}
