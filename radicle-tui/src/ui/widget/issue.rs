use radicle_cli::terminal::format;

use radicle::cob::issue::Issue;
use radicle::cob::issue::IssueId;
use radicle::Profile;

use tui_realm_stdlib::Input;
use tuirealm::props::BorderType;
use tuirealm::props::Borders;
use tuirealm::props::Color;
use tuirealm::props::Style;
use tuirealm::tui::layout::Constraint;
use tuirealm::tui::layout::Direction;
use tuirealm::tui::layout::Layout;

use super::common::container::Container;
use super::common::container::LabeledContainer;
use super::common::list::List;
use super::common::list::Property;
use super::Widget;

use crate::ui::cob;
use crate::ui::cob::IssueItem;
use crate::ui::context::Context;
use crate::ui::state::FormState;
use crate::ui::theme::Theme;
use crate::ui::widget::common::context::ContextBar;

use super::*;

pub struct LargeList {
    items: Vec<IssueItem>,
    list: Widget<LabeledContainer>,
}

impl LargeList {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
        let repo = context.repository();
        let issues = crate::cob::issue::all(repo).unwrap_or_default();
        let mut items = issues
            .iter()
            .map(|(id, issue)| IssueItem::from((context.profile(), repo, *id, issue.clone())))
            .collect::<Vec<_>>();

        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| b.state().cmp(a.state()));

        let selected =
            selected.map(|(id, issue)| IssueItem::from((context.profile(), repo, id, issue)));

        let list = Widget::new(List::new(&items, selected, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        let container = common::labeled_container(theme, "Issues", list.to_boxed());

        Self {
            items,
            list: container,
        }
    }

    pub fn items(&self) -> &Vec<IssueItem> {
        &self.items
    }
}

impl WidgetComponent for LargeList {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.list.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.list.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.list.perform(cmd)
    }
}

pub struct Details {
    container: Widget<Container>,
}

impl Details {
    pub fn new(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Self {
        let repo = context.repository();

        let (id, issue) = issue;
        let item = IssueItem::from((context.profile(), repo, id, issue));

        let title = Property::new(
            common::label("Title").foreground(theme.colors.property_name_fg),
            common::label(item.title()).foreground(theme.colors.browser_list_title),
        );

        let tags = Property::new(
            common::label("Tags").foreground(theme.colors.property_name_fg),
            common::label(&cob::format_tags(item.tags()))
                .foreground(theme.colors.browser_list_tags),
        );

        let assignees = Property::new(
            common::label("Assignees").foreground(theme.colors.property_name_fg),
            common::label(&cob::format_assignees(
                &item
                    .assignees()
                    .iter()
                    .map(|item| (item.did(), item.is_you()))
                    .collect::<Vec<_>>(),
            ))
            .foreground(theme.colors.browser_list_author),
        );

        let state = Property::new(
            common::label("Status").foreground(theme.colors.property_name_fg),
            common::label(&item.state().to_string()).foreground(theme.colors.browser_list_title),
        );

        // let table = common::property_table(theme, vec![title, tags, assignees, state]);
        let table = common::property_table(
            theme,
            vec![
                Widget::new(title),
                Widget::new(tags),
                Widget::new(assignees),
                Widget::new(state),
            ],
        );
        let container = common::container(theme, table.to_boxed());

        Self { container }
    }
}

impl WidgetComponent for Details {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        self.container.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct IssueDiscussion {
    details: Widget<Details>,
}

impl IssueDiscussion {
    pub fn new(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Self {
        Self {
            details: details(context, theme, issue),
        }
    }
}

impl WidgetComponent for IssueDiscussion {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(1)])
            .split(area);

        self.details.view(frame, layout[0]);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub struct NewForm {
    /// The issue this form writes its input values to.
    _issue: Issue,
    // This form's fields: title, tags, assignees, description.
    inputs: Vec<Input>,
    /// State that holds the current focus etc.
    state: FormState,
}

impl NewForm {
    pub fn new(_context: &Context, theme: &Theme) -> Self {
        let foreground = theme.colors.default_fg;
        let placeholder_style = Style::default().fg(theme.colors.input_placeholder_fg);
        let inactive_style = Style::default().fg(theme.colors.container_border_fg);
        let borders = Borders::default()
            .modifiers(BorderType::Rounded)
            .color(theme.colors.container_border_focus_fg);

        let title = Input::default()
            .foreground(foreground)
            .borders(borders.clone())
            .inactive(inactive_style)
            .placeholder("Title", placeholder_style);
        let tags = Input::default()
            .foreground(foreground)
            .borders(borders.clone())
            .inactive(inactive_style)
            .placeholder("Tags", placeholder_style);
        let assignees = Input::default()
            .foreground(foreground)
            .borders(borders.clone())
            .inactive(inactive_style)
            .placeholder("Assignees", placeholder_style);
        let description = Input::default()
            .foreground(foreground)
            .borders(borders)
            .inactive(inactive_style)
            .placeholder("Description", placeholder_style);

        let state = FormState::new(Some(0), 4);

        Self {
            _issue: Issue::default(),
            inputs: vec![title, tags, assignees, description],
            state,
        }
    }
}

impl WidgetComponent for NewForm {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = self.state.focus().unwrap_or(0);
        if let Some(input) = self.inputs.get_mut(focus) {
            input.attr(Attribute::Focus, AttrValue::Flag(true));
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(area);

        for (index, area) in layout.iter().enumerate().take(self.inputs.len()) {
            if let Some(input) = self.inputs.get_mut(index) {
                input.view(frame, *area);
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

pub fn list(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Widget<LargeList> {
    let list = LargeList::new(context, theme, Some(issue));

    Widget::new(list)
}

pub fn details(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Widget<Details> {
    let details = Details::new(context, theme, issue);
    Widget::new(details)
}

pub fn new_form(context: &Context, theme: &Theme) -> Widget<NewForm> {
    let form = NewForm::new(context, theme);
    Widget::new(form)
}

pub fn issue_discussion(
    context: &Context,
    theme: &Theme,
    issue: (IssueId, Issue),
) -> Widget<IssueDiscussion> {
    let discussion = IssueDiscussion::new(context, theme, issue);
    Widget::new(discussion)
}

pub fn context(theme: &Theme, issue: (IssueId, &Issue), profile: &Profile) -> Widget<ContextBar> {
    let (id, issue) = issue;
    let is_you = *issue.author().id() == profile.did();

    let id = format::cob(&id);
    let title = issue.title();
    let author = cob::format_author(issue.author().id(), is_you);
    let comments = issue.comments().count();

    let context = common::label(" issue ").background(theme.colors.context_badge_bg);
    let id = common::label(&format!(" {id} "))
        .foreground(theme.colors.context_id_fg)
        .background(theme.colors.context_id_bg);
    let title = common::label(&format!(" {title} "))
        .foreground(theme.colors.default_fg)
        .background(theme.colors.context_bg);
    let author = common::label(&format!(" {author} "))
        .foreground(theme.colors.context_id_author_fg)
        .background(theme.colors.context_bg);
    let comments = common::label(&format!(" {comments} "))
        .foreground(Color::Rgb(70, 70, 70))
        .background(theme.colors.context_light_bg);

    let context_bar = ContextBar::new(context, id, author, title, comments);

    Widget::new(context_bar).height(1)
}
