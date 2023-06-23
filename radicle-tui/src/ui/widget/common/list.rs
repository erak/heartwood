use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{
    Alignment, AttrValue, Attribute, BorderSides, BorderType, Color, Props, Style,
};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::tui::text::{Span, Spans, Text};
use tuirealm::tui::widgets::{Block, Cell, ListState, Paragraph, Row, TableState};
use tuirealm::{Frame, MockComponent, State, StateValue};

use tui_tree_widget::{MultilineTree, TreeIdentifierVec, TreeState};

use crate::ui::layout;
use crate::ui::state::ItemState;
use crate::ui::theme::Theme;
use crate::ui::widget::{utils, Widget, WidgetComponent};

use super::container::Header;
use super::label::Label;
use super::*;

/// A generic item that can be displayed in a table with [`W`] columns.
pub trait TableItem<const W: usize> {
    /// Should return fields as table cells.
    fn row(&self, theme: &Theme) -> [Cell; W];
}

/// A generic item that can be displayed in a list.
pub trait ListItem {
    /// Should return fields as list item.
    fn row(&self, theme: &Theme) -> tuirealm::tui::widgets::ListItem;
}

/// A generic item that can be displayed in a tree.
pub trait TreeItem {
    /// Should return this and its children as tree item(s), calculating
    /// some optimal height based on given [`area`] and [`items`].
    fn rows<'a>(
        &'a self,
        theme: &Theme,
        area: Option<Rect>,
        items: Option<usize>,
    ) -> Vec<tui_tree_widget::TreeItem<'a>>;

    /// Should return true if this has children.
    fn has_children(&self) -> bool;
}

/// Grow behavior of a table column.
///
/// [`tui::widgets::Table`] does only support percental column widths.
/// A [`ColumnWidth`] is used to specify the grow behaviour of a table column
/// and a percental column width is calculated based on that.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnWidth {
    /// A fixed-size column.
    Fixed(u16),
    /// A growable column.
    Grow,
}

/// A component that displays a labeled property.
#[derive(Clone)]
pub struct Property {
    name: Widget<Label>,
    divider: Widget<Label>,
    value: Widget<Label>,
}

impl Property {
    pub fn new(name: Widget<Label>, value: Widget<Label>) -> Self {
        let divider = label("");
        Self {
            name,
            divider,
            value,
        }
    }

    pub fn with_divider(mut self, divider: Widget<Label>) -> Self {
        self.divider = divider;
        self
    }

    pub fn name(&self) -> &Widget<Label> {
        &self.name
    }

    pub fn value(&self) -> &Widget<Label> {
        &self.value
    }
}

impl WidgetComponent for Property {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let labels: Vec<Box<dyn MockComponent>> = vec![
                self.name.clone().to_boxed(),
                self.divider.clone().to_boxed(),
                self.value.clone().to_boxed(),
            ];

            let layout = layout::h_stack(labels, area);
            for (mut label, area) in layout {
                label.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A component that can display lists of labeled properties
#[derive(Default)]
pub struct PropertyList {
    properties: Vec<Widget<Property>>,
}

impl PropertyList {
    pub fn new(properties: Vec<Widget<Property>>) -> Self {
        Self { properties }
    }
}

impl WidgetComponent for PropertyList {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let properties = self
                .properties
                .iter()
                .map(|property| property.clone().to_boxed() as Box<dyn MockComponent>)
                .collect();

            let layout = layout::v_stack(properties, area);
            for (mut property, area) in layout {
                property.view(frame, area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct PropertyTable {
    properties: Vec<Widget<Property>>,
}

impl PropertyTable {
    pub fn new(properties: Vec<Widget<Property>>) -> Self {
        Self { properties }
    }
}

impl WidgetComponent for PropertyTable {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tuirealm::tui::widgets::Table;

        let display = properties
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let rows = self
                .properties
                .iter()
                .map(|p| Row::new([Cell::from(p.name()), Cell::from(p.value())]));

            let table = Table::new(rows)
                .widths([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref());
            frame.render_widget(table, area);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A table component that can display a list of [`TableItem`]s hold by a [`TableModel`].
pub struct Table<V, const W: usize>
where
    V: TableItem<W> + Clone,
{
    /// Items hold by this model.
    items: Vec<V>,
    /// The table header.
    header: [Widget<Label>; W],
    /// Grow behavior of table columns.
    widths: [ColumnWidth; W],
    /// State that keeps track of the selection.
    state: ItemState,
    /// The current theme.
    theme: Theme,
}

impl<V, const W: usize> Table<V, W>
where
    V: TableItem<W> + Clone,
{
    pub fn new(
        items: &[V],
        header: [Widget<Label>; W],
        widths: [ColumnWidth; W],
        theme: Theme,
    ) -> Self {
        Self {
            items: items.to_vec(),
            header,
            widths,
            state: ItemState::new(Some(0), items.len()),
            theme,
        }
    }
}

impl<V, const W: usize> WidgetComponent for Table<V, W>
where
    V: TableItem<W> + Clone,
{
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let highlight = properties
            .get_or(Attribute::HighlightedColor, AttrValue::Color(Color::Reset))
            .unwrap_color();

        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        let color = if focus {
            self.theme.colors.container_border_focus_fg
        } else {
            self.theme.colors.container_border_fg
        };

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(3), Constraint::Min(1)])
            .split(area);

        let widths = utils::column_widths(area, &self.widths, self.theme.tables.spacing);
        let rows: Vec<Row<'_>> = self
            .items
            .iter()
            .map(|item| Row::new(item.row(&self.theme)))
            .collect();

        let table = tuirealm::tui::widgets::Table::new(rows)
            .block(
                Block::default()
                    .borders(BorderSides::BOTTOM | BorderSides::LEFT | BorderSides::RIGHT)
                    .border_style(Style::default().fg(color))
                    .border_type(BorderType::Rounded),
            )
            .highlight_style(Style::default().bg(highlight))
            .column_spacing(self.theme.tables.spacing)
            .widths(&widths);

        let mut header = Widget::new(Header::new(
            self.header.clone(),
            self.widths,
            self.theme.clone(),
        ));

        header.attr(Attribute::Focus, AttrValue::Flag(focus));
        header.view(frame, layout[0]);

        frame.render_stateful_widget(table, layout[1], &mut TableState::from(&self.state));
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;
        match cmd {
            Cmd::Move(Direction::Up) => match self.state.select_previous() {
                Some(selected) => CmdResult::Changed(State::One(StateValue::Usize(selected))),
                None => CmdResult::None,
            },
            Cmd::Move(Direction::Down) => match self.state.select_next() {
                Some(selected) => CmdResult::Changed(State::One(StateValue::Usize(selected))),
                None => CmdResult::None,
            },
            Cmd::Submit => match self.state.selected() {
                Some(selected) => CmdResult::Submit(State::One(StateValue::Usize(selected))),
                None => CmdResult::None,
            },
            _ => CmdResult::None,
        }
    }
}

/// A list component that can display [`ListItem`]'s.
pub struct List<V>
where
    V: ListItem + Clone + PartialEq,
{
    /// Items held by this list.
    items: Vec<V>,
    /// State keeps track of the current selection.
    state: ItemState,
    /// The current theme.
    theme: Theme,
}

impl<V> List<V>
where
    V: ListItem + Clone + PartialEq,
{
    pub fn new(items: &[V], selected: Option<V>, theme: Theme) -> Self {
        let selected = match selected {
            Some(item) => items.iter().position(|i| i == &item),
            None => Some(0),
        };

        Self {
            items: items.to_vec(),
            state: ItemState::new(selected, items.len()),
            theme,
        }
    }
}

impl<V> WidgetComponent for List<V>
where
    V: ListItem + Clone + PartialEq,
{
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tuirealm::tui::widgets::{List, ListItem};

        let highlight = properties
            .get_or(Attribute::HighlightedColor, AttrValue::Color(Color::Reset))
            .unwrap_color();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let rows: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| item.row(&self.theme))
            .collect();
        let list = List::new(rows).highlight_style(Style::default().bg(highlight));

        frame.render_stateful_widget(list, layout[0], &mut ListState::from(&self.state));
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;
        match cmd {
            Cmd::Move(Direction::Up) => match self.state.select_previous() {
                Some(selected) => CmdResult::Changed(State::One(StateValue::Usize(selected))),
                None => CmdResult::None,
            },
            Cmd::Move(Direction::Down) => match self.state.select_next() {
                Some(selected) => CmdResult::Changed(State::One(StateValue::Usize(selected))),
                None => CmdResult::None,
            },
            Cmd::Submit => match self.state.selected() {
                Some(selected) => CmdResult::Submit(State::One(StateValue::Usize(selected))),
                None => CmdResult::None,
            },
            _ => CmdResult::None,
        }
    }
}

/// A tree component that can display [`TreeItem`]'s.
pub struct Tree<V> {
    /// Items held by this list.
    items: Vec<V>,
    /// State keeps track of the current selection.
    state: TreeState,
    /// Count of all comments, including replies.
    count: usize,
    /// Current position in the full list.
    position: usize,
    /// The current theme.
    theme: Theme,
}

impl<V> Tree<V>
where
    V: TreeItem + Clone + PartialEq,
{
    pub fn new(
        items: &[V],
        selected: Option<TreeIdentifierVec>,
        count: usize,
        expand: bool,
        theme: Theme,
    ) -> Self {
        let mut state = TreeState::default();
        if expand {
            for (index, item) in items.iter().enumerate() {
                if item.has_children() {
                    state.open(vec![index]);
                }
            }
        }

        match selected {
            Some(identifier) => state.select(identifier),
            _ => state.select_first(),
        }

        Self {
            items: items.to_vec(),
            state,
            count,
            position: 1,
            theme,
        }
    }
}

impl<V> WidgetComponent for Tree<V>
where
    V: TreeItem + Clone + PartialEq,
{
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let mut items = vec![];
        for item in &self.items {
            items.extend(item.rows(&self.theme, Some(layout[0]), Some(self.count)));
        }

        let highlight_color = if focus {
            self.theme.colors.container_border_focus_fg
        } else {
            self.theme.colors.container_border_fg
        };

        let tree = MultilineTree::new(items)
            .item_block(
                Block::default()
                    .borders(BorderSides::ALL)
                    .border_style(Style::default().fg(self.theme.colors.container_border_fg))
                    .border_type(BorderType::Rounded),
            )
            .item_block_highlight(
                Block::default()
                    .borders(BorderSides::ALL)
                    .border_style(Style::default().fg(highlight_color))
                    .border_type(BorderType::Rounded),
            )
            .node_closed_symbol("🞂 ")
            .node_open_symbol("🞃 ");

        if focus {
            let pager = Paragraph::new(Text::from(Spans::from(Span::styled(
                format!("{} / {}", self.position, self.count),
                Style::default().fg(highlight_color),
            ))))
            .alignment(Alignment::Right);

            frame.render_widget(pager, layout[1]);
        }

        frame.render_stateful_widget(tree, layout[0], &mut self.state);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;

        let mut tree = vec![];
        for item in &self.items {
            tree.extend(item.rows(&self.theme, None, None));
        }

        match cmd {
            Cmd::Move(Direction::Up) => {
                self.position = std::cmp::max(self.position.saturating_sub(1), 1);
                self.state.key_up(&tree);
                CmdResult::None
            }
            Cmd::Move(Direction::Down) => {
                self.position = std::cmp::min(self.position.saturating_add(1), self.count);
                self.state.key_down(&tree);
                CmdResult::None
            }
            Cmd::Move(Direction::Left) => {
                let selected = self.state.selected();
                self.state.key_left();
                self.state.select(selected);
                CmdResult::None
            }
            Cmd::Move(Direction::Right) => {
                self.state.key_right();
                CmdResult::None
            }
            Cmd::Submit => {
                let values = self
                    .state
                    .selected()
                    .iter()
                    .map(|id| StateValue::Usize(*id))
                    .collect();
                CmdResult::Submit(State::Vec(values))
            }
            _ => CmdResult::None,
        }
    }
}
