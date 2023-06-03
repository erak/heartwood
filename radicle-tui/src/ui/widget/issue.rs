use radicle_cli::terminal::format;

use radicle::cob::issue::Issue;
use radicle::cob::issue::IssueId;
use radicle::Profile;
use tuirealm::props::Color;
use tuirealm::tui::layout::Constraint;
use tuirealm::tui::layout::Direction;
use tuirealm::tui::layout::Layout;

use super::common::container::LabeledContainer;
use super::common::label::Label;
use super::common::list::List;
use super::Widget;

use crate::cob;
use crate::ui::cob::IssueItem;
use crate::ui::context::Context;
use crate::ui::state;
use crate::ui::theme::Theme;
use crate::ui::widget::common::context::ContextBar;

use super::*;

pub struct LargeList {
    container: Widget<LabeledContainer>,
}

impl LargeList {
    pub fn new(context: &Context, theme: &Theme, selected: Option<(IssueId, Issue)>) -> Self {
        let repo = context.repository();

        let mut items = match cob::issue::all(repo) {
            Ok(issues) => issues
                .into_iter()
                .map(|(id, issue)| IssueItem::from((context.profile(), repo, id, issue)))
                .collect(),
            Err(_) => vec![],
        };
        items.sort_by(|a, b| b.timestamp().cmp(a.timestamp()));
        items.sort_by(|a, b| a.state().cmp(b.state()));

        let selected =
            selected.map(|(id, issue)| IssueItem::from((context.profile(), repo, id, issue)));

        let list = Widget::new(List::new(&items, selected, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        let container = common::labeled_container(theme, "Issues", list.to_boxed());

        Self { container }
    }
}

impl WidgetComponent for LargeList {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        self.container.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.container.perform(cmd)
    }
}

struct DetailHeader {
    title: Widget<Label>,
    state: Widget<Label>,
    tags: Widget<Label>,
    author: Widget<Label>,
}

impl DetailHeader {
    pub fn new(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Self {
        use crate::ui::cob;
    
        let (id, issue) = issue;

        let title = common::label(issue.title()).foreground(theme.colors.browser_list_title);
        let state = common::label(&issue.state().to_string());
        let tags = common::label(&cob::format_tags(issue.tags()));

        Self {
            title,
            state,
            tags,
            author,
        }
    }
}

impl WidgetComponent for DetailHeader {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        use tuirealm::tui::widgets::Table;
        // self.container.view(frame, area);
        let rows = vec![];
        let table = Table::new(rows);
        frame.render_widget(table, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        // self.container.perform(cmd)
        CmdResult::None
    }
}

pub struct Details {
    header: Widget<DetailHeader>,
}

impl Details {
    pub fn new(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Self {
        let header = Widget::new(DetailHeader { title: (), status: (), tags: (), author: () });

        Self { header }
    }
}

impl WidgetComponent for Details {
    fn view(&mut self, _properties: &Props, frame: &mut Frame, area: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(3), Constraint::Min(1)])
            .split(area);

        self.header.view(frame, layout[0]);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

pub fn list(context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Widget<LargeList> {
    let list = LargeList::new(context, theme, Some(issue));
    Widget::new(list)
}

pub fn context(theme: &Theme, issue: (IssueId, &Issue), profile: &Profile) -> Widget<ContextBar> {
    use crate::ui::cob;

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
