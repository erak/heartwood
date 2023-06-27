use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::Rect;
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use crate::ui::theme::Theme;
use crate::ui::widget::{Widget, WidgetComponent};

use super::container::Popup;
use super::label::Label;

pub struct WarningPopup {
    popup: Widget<Popup>,
}

impl WarningPopup {
    pub fn new(theme: Theme, label: Widget<Label>) -> Self {
        Self {
            popup: Widget::new(Popup::new(theme.clone(), label.to_boxed())),
        }
    }
}

impl WidgetComponent for WarningPopup {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            self.popup.view(frame, area);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.popup.perform(cmd)
    }
}
