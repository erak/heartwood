use std::cmp;

use radicle::cob::thread::{Comment, CommentId};
use radicle_surf;

use cli::terminal::format;
use radicle_cli as cli;

use radicle::prelude::Did;
use radicle::storage::git::Repository;
use radicle::storage::{Oid, ReadRepository};
use radicle::Profile;

use radicle::cob::issue::{Issue, IssueId, State as IssueState};
use radicle::cob::patch::{Patch, PatchId, State as PatchState};
use radicle::cob::{Tag, Timestamp};

use tuirealm::props::{Color, Style};
use tuirealm::tui::layout::{Constraint, Rect};
use tuirealm::tui::text::{Span, Spans, Text};
use tuirealm::tui::widgets::Cell;

use crate::ui::theme::Theme;
use crate::ui::widget::common::list::TableItem;

use super::widget::common::list::{ListItem, TreeItem};

/// An author item that can be used in tables, list or trees.
///
/// Breaks up dependencies to [`Profile`] and [`Repository`] that
/// would be needed if [`Author`] would be used directly.
#[derive(Clone, Debug)]
pub struct AuthorItem {
    /// The author's DID.
    did: Did,
    /// True if the author is the current user.
    is_you: bool,
}

impl AuthorItem {
    pub fn did(&self) -> Did {
        self.did
    }

    pub fn is_you(&self) -> bool {
        self.is_you
    }
}

/// A comment item that can be used in tables, list or trees.
#[derive(Clone, Debug)]
pub struct CommentItem {
    /// Comment OID.
    id: CommentId,
    /// Author of this comment.
    author: AuthorItem,
    /// The content of this comment.
    body: String,
    /// Reactions to this comment.
    reactions: Vec<char>,
    /// Time when patch was opened.
    timestamp: Timestamp,
    /// Replies to this comment.
    replies: Vec<CommentItem>,
}

impl From<(&Profile, Issue, CommentId, Comment)> for CommentItem {
    fn from(value: (&Profile, Issue, CommentId, Comment)) -> Self {
        let (profile, issue, id, comment) = value;
        let thread = issue.thread();
        let did = Did::from(comment.author());

        CommentItem {
            id,
            author: AuthorItem {
                did,
                is_you: did == profile.did(),
            },
            body: comment.body().to_string(),
            reactions: thread
                .reactions(&id)
                .map(|(_, reaction)| reaction.emoji())
                .collect(),
            timestamp: comment.timestamp(),
            replies: thread
                .replies(&id)
                .map(|(reply_id, reply)| {
                    CommentItem::from((profile, issue.clone(), *reply_id, reply.clone()))
                })
                .collect(),
        }
    }
}

impl CommentItem {
    pub fn id(&self) -> &CommentId {
        &self.id
    }

    pub fn replies(&self) -> &Vec<CommentItem> {
        &self.replies
    }
}

impl TreeItem for CommentItem {
    fn rows<'a>(
        &'a self,
        theme: &Theme,
        area: Option<Rect>,
        items: Option<usize>,
    ) -> Vec<tui_tree_widget::TreeItem<'a>> {
        let area = area.unwrap_or_default();
        let available = area.height.saturating_sub(1) as usize;

        let items_per_page = match items {
            Some(items) => cmp::min(items, 6),
            None => 1,
        };
        let items_per_page = if area.height < 20 {
            cmp::min(items_per_page, 1)
        } else if area.height < 30 {
            cmp::min(items_per_page, 2)
        } else {
            items_per_page
        };

        let heights = vec![
            Constraint::Length(1),
            Constraint::Length(
                cmp::min(
                    available.saturating_div(items_per_page).saturating_sub(2),
                    available.saturating_sub(1),
                )
                .saturating_sub(3) as u16,
            ),
            Constraint::Length(1),
            Constraint::Length(1),
        ];

        let meta = Spans::from(vec![
            Span::styled(
                format_author(&self.author.did, self.author.is_you),
                Style::default().fg(theme.colors.browser_list_author),
            ),
            Span::styled(
                format!(" {} ", theme.icons.property_divider),
                Style::default().fg(theme.colors.property_divider_fg),
            ),
            Span::styled(
                format::timestamp(&self.timestamp).to_string(),
                Style::default().fg(theme.colors.browser_list_timestamp),
            ),
            Span::styled(
                format!(" {} ", theme.icons.property_divider),
                Style::default().fg(theme.colors.property_divider_fg),
            ),
            Span::styled(
                format!(
                    "{} {}",
                    self.replies.len(),
                    if self.replies.len() == 1_usize {
                        "reply"
                    } else {
                        "replies"
                    }
                ),
                Style::default().fg(theme.colors.browser_list_comments),
            ),
        ]);

        let mut body: Text<'_> = self.body.clone().into();
        for line in &mut body.lines {
            for mut span in &mut line.0 {
                span.style = Style::default().fg(theme.colors.default_fg);
            }
        }

        let reactions = self
            .reactions
            .iter()
            .map(|emoji| Span::raw(format!("{emoji}")))
            .collect::<Vec<_>>();
        let reactions = Text::from(Spans::from(reactions));

        let mut children = vec![];
        for comment in &self.replies {
            children.extend(comment.rows(theme, Some(area), items));
        }

        vec![tui_tree_widget::TreeItem::new(meta, children)
            .paragraph(body)
            .paragraph(Text::from(Spans::from(vec![])))
            .paragraph(reactions)
            .heights(&heights)]
    }

    fn has_children(&self) -> bool {
        !self.replies.is_empty()
    }
}

impl PartialEq for CommentItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// A patch item that can be used in tables, list or trees.
///
/// Breaks up dependencies to [`Profile`] and [`Repository`] that
/// would be needed if [`Patch`] would be used directly.
#[derive(Clone)]
pub struct PatchItem {
    /// Patch OID.
    id: PatchId,
    /// Patch state.
    state: PatchState,
    /// Patch title.
    title: String,
    /// Author of the latest revision.
    author: AuthorItem,
    /// Head of the latest revision.
    head: Oid,
    /// Lines added by the latest revision.
    added: u16,
    /// Lines removed by the latest revision.
    removed: u16,
    /// Time when patch was opened.
    timestamp: Timestamp,
}

impl PatchItem {
    pub fn id(&self) -> &PatchId {
        &self.id
    }

    pub fn state(&self) -> &PatchState {
        &self.state
    }

    pub fn title(&self) -> &String {
        &self.title
    }

    pub fn author(&self) -> &AuthorItem {
        &self.author
    }

    pub fn head(&self) -> &Oid {
        &self.head
    }

    pub fn added(&self) -> u16 {
        self.added
    }

    pub fn removed(&self) -> u16 {
        self.removed
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }
}

impl TryFrom<(&Profile, &Repository, PatchId, Patch)> for PatchItem {
    type Error = anyhow::Error;

    fn try_from(value: (&Profile, &Repository, PatchId, Patch)) -> Result<Self, Self::Error> {
        let (profile, repo, id, patch) = value;
        let (_, rev) = patch.latest();
        let repo = radicle_surf::Repository::open(repo.path())?;
        let base = repo.commit(rev.base())?;
        let head = repo.commit(rev.head())?;
        let diff = repo.diff(base.id, head.id)?;

        Ok(PatchItem {
            id,
            state: patch.state().clone(),
            title: patch.title().into(),
            author: AuthorItem {
                did: patch.author().id,
                is_you: *patch.author().id == *profile.did(),
            },
            head: rev.head(),
            added: diff.stats().insertions as u16,
            removed: diff.stats().deletions as u16,
            timestamp: rev.timestamp(),
        })
    }
}

impl TableItem<8> for PatchItem {
    fn row(&self, theme: &Theme) -> [Cell; 8] {
        let (icon, color) = format_patch_state(&self.state);
        let state = Cell::from(icon).style(Style::default().fg(color));

        let id = Cell::from(format::cob(&self.id))
            .style(Style::default().fg(theme.colors.browser_list_id));

        let title = Cell::from(self.title.clone())
            .style(Style::default().fg(theme.colors.browser_list_title));

        let author = Cell::from(format_author(&self.author.did, self.author.is_you))
            .style(Style::default().fg(theme.colors.browser_list_author));

        let head = Cell::from(format::oid(self.head).item)
            .style(Style::default().fg(theme.colors.browser_patch_list_head));

        let added = Cell::from(format!("{}", self.added))
            .style(Style::default().fg(theme.colors.browser_patch_list_added));

        let removed = Cell::from(format!("{}", self.removed))
            .style(Style::default().fg(theme.colors.browser_patch_list_removed));

        let updated = Cell::from(format::timestamp(&self.timestamp).to_string())
            .style(Style::default().fg(theme.colors.browser_list_timestamp));

        [state, id, title, author, head, added, removed, updated]
    }
}

/// An issue item that can be used in tables, list or trees.
///
/// Breaks up dependencies to [`Profile`] and [`Repository`] that
/// would be needed if [`Issue`] would be used directly.
#[derive(Clone)]
pub struct IssueItem {
    /// Issue OID.
    id: IssueId,
    /// Issue state.
    state: IssueState,
    /// Issue title.
    title: String,
    /// Issue author.
    author: AuthorItem,
    /// Issue tags.
    tags: Vec<Tag>,
    /// Issue assignees.
    assignees: Vec<AuthorItem>,
    /// Time when issue was opened.
    timestamp: Timestamp,
}

impl IssueItem {
    pub fn id(&self) -> &IssueId {
        &self.id
    }

    pub fn state(&self) -> &IssueState {
        &self.state
    }

    pub fn title(&self) -> &String {
        &self.title
    }

    pub fn author(&self) -> &AuthorItem {
        &self.author
    }

    pub fn tags(&self) -> &Vec<Tag> {
        &self.tags
    }

    pub fn assignees(&self) -> &Vec<AuthorItem> {
        &self.assignees
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }
}

impl From<(&Profile, &Repository, IssueId, Issue)> for IssueItem {
    fn from(value: (&Profile, &Repository, IssueId, Issue)) -> Self {
        let (profile, _, id, issue) = value;

        IssueItem {
            id,
            state: *issue.state(),
            title: issue.title().into(),
            author: AuthorItem {
                did: issue.author().id,
                is_you: *issue.author().id == *profile.did(),
            },
            tags: issue.tags().cloned().collect(),
            assignees: issue
                .assigned()
                .map(|did| AuthorItem {
                    did,
                    is_you: did == profile.did(),
                })
                .collect::<Vec<_>>(),
            timestamp: issue.timestamp(),
        }
    }
}

impl TableItem<7> for IssueItem {
    fn row(&self, theme: &Theme) -> [Cell; 7] {
        let (icon, color) = format_issue_state(&self.state);
        let state = Cell::from(icon).style(Style::default().fg(color));

        let id = Cell::from(format::cob(&self.id))
            .style(Style::default().fg(theme.colors.browser_list_id));

        let title = Cell::from(self.title.clone())
            .style(Style::default().fg(theme.colors.browser_list_title));

        let author = Cell::from(format_author(&self.author.did, self.author.is_you))
            .style(Style::default().fg(theme.colors.browser_list_author));

        let tags = Cell::from(format_tags(&self.tags))
            .style(Style::default().fg(theme.colors.browser_list_tags));

        let assignees = self
            .assignees
            .iter()
            .map(|author| (author.did, author.is_you))
            .collect::<Vec<_>>();
        let assignees = Cell::from(format_assignees(&assignees))
            .style(Style::default().fg(theme.colors.browser_list_author));

        let opened = Cell::from(format::timestamp(&self.timestamp).to_string())
            .style(Style::default().fg(theme.colors.browser_list_timestamp));

        [state, id, title, author, tags, assignees, opened]
    }
}

impl ListItem for IssueItem {
    fn row(&self, theme: &Theme) -> tuirealm::tui::widgets::ListItem {
        let (state, state_color) = format_issue_state(&self.state);
        let lines = vec![
            Spans::from(vec![
                Span::styled(state, Style::default().fg(state_color)),
                Span::styled(
                    self.title.clone(),
                    Style::default().fg(theme.colors.browser_list_title),
                ),
            ]),
            Spans::from(vec![
                Span::raw(String::from("   ")),
                Span::styled(
                    format_author(&self.author.did, self.author.is_you),
                    Style::default().fg(theme.colors.browser_list_author),
                ),
                Span::styled(
                    format!(" {} ", theme.icons.property_divider),
                    Style::default().fg(theme.colors.property_divider_fg),
                ),
                Span::styled(
                    format::timestamp(&self.timestamp).to_string(),
                    Style::default().fg(theme.colors.browser_list_timestamp),
                ),
            ]),
        ];
        tuirealm::tui::widgets::ListItem::new(lines)
    }
}

impl PartialEq for IssueItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub fn format_patch_state(state: &PatchState) -> (String, Color) {
    match state {
        PatchState::Open { conflicts: _ } => (" ● ".into(), Color::Green),
        PatchState::Archived => (" ● ".into(), Color::Yellow),
        PatchState::Draft => (" ● ".into(), Color::Gray),
        PatchState::Merged {
            revision: _,
            commit: _,
        } => (" ✔ ".into(), Color::Blue),
    }
}

pub fn format_author(did: &Did, is_you: bool) -> String {
    if is_you {
        format!("{} (you)", format::did(did))
    } else {
        format!("{}", format::did(did))
    }
}

pub fn format_issue_state(state: &IssueState) -> (String, Color) {
    match state {
        IssueState::Open => (" ● ".into(), Color::Green),
        IssueState::Closed { reason: _ } => (" ● ".into(), Color::Red),
    }
}

pub fn format_tags(tags: &[Tag]) -> String {
    let mut output = String::new();
    let mut tags = tags.iter().peekable();

    while let Some(tag) = tags.next() {
        output.push_str(&tag.to_string());

        if tags.peek().is_some() {
            output.push(',');
        }
    }
    output
}

pub fn format_assignees(assignees: &[(Did, bool)]) -> String {
    let mut output = String::new();
    let mut assignees = assignees.iter().peekable();

    while let Some((assignee, is_you)) = assignees.next() {
        output.push_str(&format_author(assignee, *is_you));

        if assignees.peek().is_some() {
            output.push(',');
        }
    }
    output
}
