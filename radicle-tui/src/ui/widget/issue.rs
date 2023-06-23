use radicle::cob::thread::Comment;
use radicle::cob::thread::CommentId;
use radicle_cli::terminal::format;

use radicle::cob::issue::Issue;
use radicle::cob::issue::IssueId;
use radicle::Profile;
use tui_tree_widget::TreeIdentifierVec;
use tuirealm::props::Color;
use tuirealm::StateValue;

use super::common::container::Container;
use super::common::container::LabeledContainer;
use super::common::list::List;
use super::common::list::Property;
use super::common::list::Tree;
use super::Widget;

use crate::ui::cob;
use crate::ui::cob::CommentItem;
use crate::ui::cob::IssueItem;
use crate::ui::context::Context;
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
        items.sort_by(|a, b| a.state().cmp(b.state()));

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

pub struct Discussion {
    /// All comments
    all: Vec<CommentItem>,
    /// First level items in comment tree.
    root: Vec<CommentItem>,
    /// Tree widget without borders.
    tree: Widget<Tree<CommentItem>>,
}

impl Discussion {
    pub fn new(
        context: &Context,
        theme: &Theme,
        issue: (IssueId, Issue),
        selected: Option<(CommentId, Comment)>,
    ) -> Self {
        let (_, issue) = issue;
        let count = issue.comments().count();
        let all = issue
            .comments()
            .map(|(id, comment)| {
                CommentItem::from((context.profile(), issue.clone(), *id, comment.clone()))
            })
            .collect::<Vec<_>>();
        let root = issue
            .comments()
            .filter(|(_, comment)| comment.reply_to().is_none())
            .map(|(id, comment)| {
                CommentItem::from((context.profile(), issue.clone(), *id, comment.clone()))
            })
            .collect::<Vec<_>>();

        let selected = selected.map(|(id, comment)| {
            CommentItem::from((context.profile(), issue.clone(), id, comment))
        });
        let selected = selected
            .as_ref()
            .map(|selected| Self::get_identifier(&root, selected, vec![]));

        let tree = Widget::new(Tree::new(&root, selected, count, true, theme.clone()))
            .highlight(theme.colors.item_list_highlighted_bg);

        Self { all, root, tree }
    }

    pub fn items(&self) -> &Vec<CommentItem> {
        &self.all
    }

    fn get_identifier(
        items: &[CommentItem],
        selected: &CommentItem,
        parents: TreeIdentifierVec,
    ) -> TreeIdentifierVec {
        let mut identifier = vec![];

        for (i, item) in items.iter().enumerate() {
            if item == selected {
                identifier.extend(parents);
                identifier.push(i);

                break;
            } else if !item.replies().is_empty() {
                let mut parents = parents.clone();

                parents.push(i);
                identifier.extend(Self::get_identifier(item.replies(), selected, parents));
            }
        }
        identifier
    }
}

impl WidgetComponent for Discussion {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.tree.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.tree.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        let result = self.tree.perform(cmd);
        match result {
            CmdResult::Submit(State::Vec(identifiers)) => {
                let identifiers = identifiers
                    .into_iter()
                    .map(|value| value.unwrap_usize())
                    .collect::<Vec<_>>();

                let mut iter = identifiers.iter();
                let mut comment = self.root.get(*iter.next().unwrap_or(&0));
                for id in iter {
                    comment = comment.unwrap().replies().get(*id);
                }

                let position = match comment {
                    Some(comment) => self.all.iter().position(|c| *c == *comment).unwrap_or(0),
                    _ => 0,
                };

                CmdResult::Submit(State::One(StateValue::Usize(position)))
            }
            _ => result,
        }
    }
}

pub struct IssueDiscussion {
    discussion: Widget<Discussion>,
    issue: (IssueId, Issue),
}

impl IssueDiscussion {
    pub fn new(
        context: &Context,
        theme: &Theme,
        issue: (IssueId, Issue),
        selected: Option<(CommentId, Comment)>,
    ) -> Self {
        let discussion = Discussion::new(context, theme, issue.clone(), selected);

        Self {
            discussion: Widget::new(discussion),
            issue,
        }
    }

    pub fn comments(&self) -> &Vec<CommentItem> {
        self.discussion.items()
    }

    pub fn issue(&self) -> &(IssueId, Issue) {
        &self.issue
    }
}

impl WidgetComponent for IssueDiscussion {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.discussion.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.discussion.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.discussion.perform(cmd)
    }
}

pub struct CommentDiscussion {
    discussion: Widget<Discussion>,
}

impl CommentDiscussion {
    pub fn new(
        context: &Context,
        theme: &Theme,
        issue: (IssueId, Issue),
        selected: Option<(CommentId, Comment)>,
    ) -> Self {
        let discussion = Discussion::new(context, theme, issue, selected);

        Self {
            discussion: Widget::new(discussion),
        }
    }

    pub fn comments(&self) -> &Vec<CommentItem> {
        self.discussion.items()
    }
}

impl WidgetComponent for CommentDiscussion {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        let focus = properties
            .get_or(Attribute::Focus, AttrValue::Flag(false))
            .unwrap_flag();

        self.discussion.attr(Attribute::Focus, AttrValue::Flag(focus));
        self.discussion.view(frame, area);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        self.discussion.perform(cmd)
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

pub fn issue_discussion(
    context: &Context,
    theme: &Theme,
    issue: (IssueId, Issue),
) -> Widget<IssueDiscussion> {
    let discussion = IssueDiscussion::new(context, theme, issue, None);
    Widget::new(discussion)
}

pub fn comment_discussion(
    context: &Context,
    theme: &Theme,
    issue: (IssueId, Issue),
    comment: Option<(CommentId, Comment)>,
) -> Widget<CommentDiscussion> {
    let discussion = CommentDiscussion::new(context, theme, issue, comment);
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
