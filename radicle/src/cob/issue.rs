use std::collections::BTreeSet;
use std::ops::Deref;
use std::str::FromStr;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cob;
use crate::cob::common::{Author, Label, Reaction, Timestamp, Uri};
use crate::cob::store::Transaction;
use crate::cob::store::{FromHistory as _, HistoryAction};
use crate::cob::thread;
use crate::cob::thread::{CommentId, Thread};
use crate::cob::{store, Embed, EntryId, ObjectId, TypeName};
use crate::crypto::Signer;
use crate::prelude::{Did, ReadRepository};
use crate::storage::WriteRepository;

/// Issue operation.
pub type Op = cob::Op<Action>;

/// Type name of an issue.
pub static TYPENAME: Lazy<TypeName> =
    Lazy::new(|| FromStr::from_str("xyz.radicle.issue").expect("type name is valid"));

/// Identifier for an issue.
pub type IssueId = ObjectId;

/// Error updating or creating issues.
#[derive(Error, Debug)]
pub enum Error {
    #[error("apply failed")]
    Apply,
    #[error("validation failed: {0}")]
    Validate(&'static str),
    #[error("description missing")]
    DescriptionMissing,
    #[error("thread apply failed: {0}")]
    Thread(#[from] thread::Error),
    #[error("store: {0}")]
    Store(#[from] store::Error),
}

/// Reason why an issue was closed.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseReason {
    Other,
    Solved,
}

impl std::fmt::Display for CloseReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::Other => "unspecified",
            Self::Solved => "solved",
        };
        write!(f, "{reason}")
    }
}

/// Issue state.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum State {
    /// The issue is closed.
    Closed { reason: CloseReason },
    /// The issue is open.
    #[default]
    Open,
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed { .. } => write!(f, "closed"),
            Self::Open => write!(f, "open"),
        }
    }
}

impl State {
    pub fn lifecycle_message(self) -> String {
        match self {
            Self::Open => "Open issue".to_owned(),
            Self::Closed { .. } => "Close issue".to_owned(),
        }
    }
}

/// Issue state. Accumulates [`Action`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Issue {
    /// Actors assigned to this issue.
    pub(super) assignees: BTreeSet<Did>,
    /// Title of the issue.
    pub(super) title: String,
    /// Current state of the issue.
    pub(super) state: State,
    /// Associated labels.
    pub(super) labels: BTreeSet<Label>,
    /// Discussion around this issue.
    pub(super) thread: Thread,
}

impl store::FromHistory for Issue {
    type Action = Action;
    type Error = Error;

    fn type_name() -> &'static TypeName {
        &TYPENAME
    }

    fn validate(&self) -> Result<(), Self::Error> {
        if self.title.is_empty() {
            return Err(Error::Validate("title is empty"));
        }
        if self.thread.validate().is_err() {
            return Err(Error::Validate("invalid thread"));
        }
        Ok(())
    }

    fn from_history<R: ReadRepository>(
        history: &radicle_cob::History,
        repo: &R,
    ) -> Result<Self, Self::Error> {
        let root = history.root();

        // Deprecated. Remove when we drop legacy support.
        if root.manifest().is_legacy() {
            let legacy = super::legacy::issue::Issue::from_history(history, repo)?;
            let issue = legacy.into();

            Ok(issue)
        } else {
            store::from_history::<R, Self>(history, repo)
        }
    }

    fn apply<R: ReadRepository>(&mut self, op: Op, _repo: &R) -> Result<(), Error> {
        for action in op.actions {
            match action {
                Action::Assign { assignees } => {
                    self.assignees = BTreeSet::from_iter(assignees);
                }
                Action::Edit { title } => {
                    self.title = title;
                }
                Action::Lifecycle { state } => {
                    self.state = state;
                }
                Action::Label { labels } => {
                    self.labels = BTreeSet::from_iter(labels);
                }
                Action::Comment {
                    body,
                    reply_to,
                    embeds,
                } => {
                    thread::comment(
                        &mut self.thread,
                        op.id,
                        op.author,
                        op.timestamp,
                        body,
                        reply_to,
                        None,
                        embeds,
                    )?;
                }
                Action::CommentEdit { id, body, embeds } => {
                    thread::edit(&mut self.thread, op.id, id, op.timestamp, body, embeds)?;
                }
                Action::CommentRedact { id } => {
                    thread::redact(&mut self.thread, op.id, id)?;
                }
                Action::CommentReact {
                    id,
                    reaction,
                    active,
                } => {
                    thread::react(&mut self.thread, op.id, op.author, id, reaction, active)?;
                }
            }
        }
        Ok(())
    }
}

impl Issue {
    pub fn assigned(&self) -> impl Iterator<Item = &Did> + '_ {
        self.assignees.iter()
    }

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn labels(&self) -> impl Iterator<Item = &Label> {
        self.labels.iter()
    }

    pub fn timestamp(&self) -> Timestamp {
        self.thread
            .comments()
            .next()
            .map(|(_, c)| c)
            .expect("Issue::timestamp: at least one comment is present")
            .timestamp()
    }

    pub fn author(&self) -> Author {
        self.thread
            .comments()
            .next()
            .map(|(_, c)| Author::new(c.author()))
            .expect("Issue::author: at least one comment is present")
    }

    pub fn description(&self) -> (&CommentId, &str) {
        self.thread
            .comments()
            .next()
            .map(|(id, c)| (id, c.body()))
            .expect("Issue::description: at least one comment is present")
    }

    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    pub fn comments(&self) -> impl Iterator<Item = (&CommentId, &thread::Comment)> {
        self.thread.comments()
    }
}

impl Deref for Issue {
    type Target = Thread;

    fn deref(&self) -> &Self::Target {
        &self.thread
    }
}

impl store::Transaction<Issue> {
    /// Assign DIDs to the issue.
    pub fn assign(&mut self, assignees: impl IntoIterator<Item = Did>) -> Result<(), store::Error> {
        self.push(Action::Assign {
            assignees: assignees.into_iter().collect(),
        })
    }

    /// Edit an issue comment.
    pub fn edit_comment(
        &mut self,
        id: CommentId,
        body: impl ToString,
        embeds: Vec<Embed>,
    ) -> Result<(), store::Error> {
        let hashed = embeds.iter().map(|e| e.hashed()).collect();

        self.embed(embeds)?;
        self.push(Action::CommentEdit {
            id,
            body: body.to_string(),
            embeds: hashed,
        })
    }

    /// Set the issue title.
    pub fn edit(&mut self, title: impl ToString) -> Result<(), store::Error> {
        self.push(Action::Edit {
            title: title.to_string(),
        })
    }

    /// Lifecycle an issue.
    pub fn lifecycle(&mut self, state: State) -> Result<(), store::Error> {
        self.push(Action::Lifecycle { state })
    }

    /// Comment on an issue.
    pub fn comment<S: ToString>(
        &mut self,
        body: S,
        reply_to: CommentId,
        embeds: Vec<Embed>,
    ) -> Result<(), store::Error> {
        let hashed = embeds.iter().map(|e| e.hashed()).collect();

        self.embed(embeds)?;
        self.push(Action::Comment {
            body: body.to_string(),
            reply_to: Some(reply_to),
            embeds: hashed,
        })
    }

    /// Label an issue.
    pub fn label(&mut self, labels: impl IntoIterator<Item = Label>) -> Result<(), store::Error> {
        self.push(Action::Label {
            labels: labels.into_iter().collect(),
        })
    }

    /// React to an issue comment.
    pub fn react(
        &mut self,
        id: CommentId,
        reaction: Reaction,
        active: bool,
    ) -> Result<(), store::Error> {
        self.push(Action::CommentReact {
            id,
            reaction,
            active,
        })
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////

    /// Create the issue thread.
    fn thread<S: ToString>(
        &mut self,
        body: S,
        embeds: impl IntoIterator<Item = Embed>,
    ) -> Result<(), store::Error> {
        let embeds = embeds.into_iter().collect::<Vec<_>>();
        let hashed = embeds.iter().map(|e| e.hashed()).collect();

        self.embed(embeds)?;
        self.push(Action::Comment {
            body: body.to_string(),
            reply_to: None,
            embeds: hashed,
        })
    }
}

pub struct IssueMut<'a, 'g, R> {
    id: ObjectId,
    issue: Issue,
    store: &'g mut Issues<'a, R>,
}

impl<'a, 'g, R> std::fmt::Debug for IssueMut<'a, 'g, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("IssueMut")
            .field("id", &self.id)
            .field("issue", &self.issue)
            .finish()
    }
}

impl<'a, 'g, R> IssueMut<'a, 'g, R>
where
    R: WriteRepository + cob::Store,
{
    /// Reload the issue data from storage.
    pub fn reload(&mut self) -> Result<(), store::Error> {
        self.issue = self
            .store
            .get(&self.id)?
            .ok_or_else(|| store::Error::NotFound(TYPENAME.clone(), self.id))?;

        Ok(())
    }

    /// Get the issue id.
    pub fn id(&self) -> &ObjectId {
        &self.id
    }

    /// Assign one or more actors to an issue.
    pub fn assign<G: Signer>(
        &mut self,
        assignees: impl IntoIterator<Item = Did>,
        signer: &G,
    ) -> Result<EntryId, Error> {
        self.transaction("Assign", signer, |tx| tx.assign(assignees))
    }

    /// Set the issue title.
    pub fn edit<G: Signer>(&mut self, title: impl ToString, signer: &G) -> Result<EntryId, Error> {
        self.transaction("Edit", signer, |tx| tx.edit(title))
    }

    /// Set the issue description.
    pub fn edit_description<G: Signer>(
        &mut self,
        description: impl ToString,
        embeds: impl IntoIterator<Item = Embed>,
        signer: &G,
    ) -> Result<EntryId, Error> {
        let Some((id, _)) = self.thread.comments().next() else {
            return Err(Error::DescriptionMissing);
        };
        let id = *id;
        self.transaction("Edit description", signer, |tx| {
            tx.edit_comment(id, description, embeds.into_iter().collect())
        })
    }

    /// Lifecycle an issue.
    pub fn lifecycle<G: Signer>(&mut self, state: State, signer: &G) -> Result<EntryId, Error> {
        self.transaction("Lifecycle", signer, |tx| tx.lifecycle(state))
    }

    /// Comment on an issue.
    pub fn comment<G: Signer, S: ToString>(
        &mut self,
        body: S,
        reply_to: CommentId,
        embeds: impl IntoIterator<Item = Embed>,
        signer: &G,
    ) -> Result<EntryId, Error> {
        assert!(
            self.thread.comment(&reply_to).is_some(),
            "Comment {reply_to} not found"
        );
        self.transaction("Comment", signer, |tx| {
            tx.comment(body, reply_to, embeds.into_iter().collect())
        })
    }

    /// Label an issue.
    pub fn label<G: Signer>(
        &mut self,
        labels: impl IntoIterator<Item = Label>,
        signer: &G,
    ) -> Result<EntryId, Error> {
        self.transaction("Label", signer, |tx| tx.label(labels))
    }

    /// React to an issue comment.
    pub fn react<G: Signer>(
        &mut self,
        to: CommentId,
        reaction: Reaction,
        active: bool,
        signer: &G,
    ) -> Result<EntryId, Error> {
        self.transaction("React", signer, |tx| tx.react(to, reaction, active))
    }

    pub fn transaction<G, F>(
        &mut self,
        message: &str,
        signer: &G,
        operations: F,
    ) -> Result<EntryId, Error>
    where
        G: Signer,
        F: FnOnce(&mut Transaction<Issue>) -> Result<(), store::Error>,
    {
        let mut tx = Transaction::new(*signer.public_key());
        operations(&mut tx)?;
        let (ops, commit) = tx.commit(message, self.id, &mut self.store.raw, signer)?;

        self.issue.apply(ops, self.store.as_ref())?;

        Ok(commit)
    }
}

impl<'a, 'g, R> Deref for IssueMut<'a, 'g, R> {
    type Target = Issue;

    fn deref(&self) -> &Self::Target {
        &self.issue
    }
}

pub struct Issues<'a, R> {
    raw: store::Store<'a, Issue, R>,
}

impl<'a, R> Deref for Issues<'a, R> {
    type Target = store::Store<'a, Issue, R>;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

/// Detailed information on issue states
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueCounts {
    pub open: usize,
    pub closed: usize,
}

impl<'a, R: WriteRepository> Issues<'a, R>
where
    R: ReadRepository + cob::Store,
{
    /// Open an issues store.
    pub fn open(repository: &'a R) -> Result<Self, store::Error> {
        let raw = store::Store::open(repository)?;

        Ok(Self { raw })
    }

    /// Get an issue.
    pub fn get(&self, id: &ObjectId) -> Result<Option<Issue>, store::Error> {
        self.raw.get(id)
    }

    /// Get an issue mutably.
    pub fn get_mut<'g>(&'g mut self, id: &ObjectId) -> Result<IssueMut<'a, 'g, R>, store::Error> {
        let issue = self
            .raw
            .get(id)?
            .ok_or_else(move || store::Error::NotFound(TYPENAME.clone(), *id))?;

        Ok(IssueMut {
            id: *id,
            issue,
            store: self,
        })
    }

    /// Create a new issue.
    pub fn create<'g, G: Signer>(
        &'g mut self,
        title: impl ToString,
        description: impl ToString,
        labels: &[Label],
        assignees: &[Did],
        embeds: impl IntoIterator<Item = Embed>,
        signer: &G,
    ) -> Result<IssueMut<'a, 'g, R>, Error> {
        let (id, issue) = Transaction::initial("Create issue", &mut self.raw, signer, |tx| {
            tx.thread(description, embeds)?;
            tx.assign(assignees.to_owned())?;
            tx.edit(title)?;
            tx.label(labels.to_owned())?;

            Ok(())
        })?;

        Ok(IssueMut {
            id,
            issue,
            store: self,
        })
    }

    /// Issues count by state.
    pub fn counts(&self) -> Result<IssueCounts, Error> {
        let all = self.all()?;
        let state_groups =
            all.filter_map(|s| s.ok())
                .fold(IssueCounts::default(), |mut state, (_, p)| {
                    match p.state() {
                        State::Open => state.open += 1,
                        State::Closed { .. } => state.closed += 1,
                    }
                    state
                });

        Ok(state_groups)
    }

    /// Remove an issue.
    pub fn remove<G: Signer>(&self, id: &ObjectId, signer: &G) -> Result<(), store::Error> {
        self.raw.remove(id, signer)
    }
}

/// Issue action.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Assign issue to an actor.
    #[serde(rename = "assign")]
    Assign { assignees: BTreeSet<Did> },

    /// Edit issue title.
    #[serde(rename = "edit")]
    Edit { title: String },

    /// Transition to a different state.
    #[serde(rename = "lifecycle")]
    Lifecycle { state: State },

    /// Modify issue labels.
    #[serde(rename = "label")]
    Label { labels: BTreeSet<Label> },

    /// Comment on a thread.
    #[serde(rename_all = "camelCase")]
    #[serde(rename = "comment")]
    Comment {
        /// Comment body.
        body: String,
        /// Comment this is a reply to.
        /// Should be [`None`] if it's the top-level comment.
        /// Should be the root [`CommentId`] if it's a top-level comment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reply_to: Option<CommentId>,
        /// Embeded content.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        embeds: Vec<Embed<Uri>>,
    },

    /// Edit a comment.
    #[serde(rename = "comment.edit")]
    CommentEdit {
        /// Comment being edited.
        id: CommentId,
        /// New value for the comment body.
        body: String,
        /// New value for the embeds list.
        embeds: Vec<Embed<Uri>>,
    },

    /// Redact a change. Not all changes can be redacted.
    #[serde(rename = "comment.redact")]
    CommentRedact { id: CommentId },

    /// React to a comment.
    #[serde(rename = "comment.react")]
    CommentReact {
        id: CommentId,
        reaction: Reaction,
        active: bool,
    },
}

impl HistoryAction for Action {}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::cob::{ActorId, Reaction};
    use crate::git::Oid;
    use crate::test;
    use crate::test::arbitrary;

    #[test]
    fn test_concurrency() {
        let t = test::setup::Network::default();
        let mut issues_alice = Issues::open(&*t.alice.repo).unwrap();
        let mut bob_issues = Issues::open(&*t.bob.repo).unwrap();
        let mut eve_issues = Issues::open(&*t.eve.repo).unwrap();

        let mut issue_alice = issues_alice
            .create(
                "Alice Issue",
                "Alice's comment",
                &[],
                &[],
                [],
                &t.alice.signer,
            )
            .unwrap();
        let id = *issue_alice.id();

        t.bob.repo.fetch(&t.alice);
        t.eve.repo.fetch(&t.alice);

        let mut issue_eve = eve_issues.get_mut(&id).unwrap();
        let mut issue_bob = bob_issues.get_mut(&id).unwrap();

        issue_bob
            .comment("Bob's reply", id.into(), vec![], &t.bob.signer)
            .unwrap();
        issue_alice
            .comment("Alice's reply", id.into(), vec![], &t.alice.signer)
            .unwrap();

        assert_eq!(issue_bob.comments().count(), 2);
        assert_eq!(issue_alice.comments().count(), 2);

        t.bob.repo.fetch(&t.alice);
        issue_bob.reload().unwrap();
        assert_eq!(issue_bob.comments().count(), 3);

        t.alice.repo.fetch(&t.bob);
        issue_alice.reload().unwrap();
        assert_eq!(issue_alice.comments().count(), 3);

        let bob_comments = issue_bob
            .comments()
            .map(|(_, c)| c.body())
            .collect::<Vec<_>>();
        let alice_comments = issue_alice
            .comments()
            .map(|(_, c)| c.body())
            .collect::<Vec<_>>();

        assert_eq!(bob_comments, alice_comments);

        t.eve.repo.fetch(&t.alice);

        let eve_reply = issue_eve
            .comment("Eve's reply", id.into(), vec![], &t.eve.signer)
            .unwrap();

        t.bob.repo.fetch(&t.eve);
        t.alice.repo.fetch(&t.eve);

        issue_alice.reload().unwrap();
        issue_bob.reload().unwrap();
        issue_eve.reload().unwrap();

        assert_eq!(issue_eve.comments().count(), 4);
        assert_eq!(issue_bob.comments().count(), 4);
        assert_eq!(issue_alice.comments().count(), 4);

        let (first, _) = issue_bob.comments().next().unwrap();
        let (last, _) = issue_bob.comments().last().unwrap();

        assert_eq!(*first, issue_alice.id.into());
        assert_eq!(*last, eve_reply);
    }

    #[test]
    fn test_ordering() {
        assert!(CloseReason::Solved > CloseReason::Other);
        assert!(
            State::Open
                > State::Closed {
                    reason: CloseReason::Solved
                }
        );
    }

    #[test]
    fn test_issue_create_and_assign() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();

        let assignee = Did::from(arbitrary::gen::<ActorId>(1));
        let assignee_two = Did::from(arbitrary::gen::<ActorId>(1));
        let issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[assignee],
                [],
                &node.signer,
            )
            .unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let assignees: Vec<_> = issue.assigned().cloned().collect::<Vec<_>>();

        assert_eq!(1, assignees.len());
        assert!(assignees.contains(&assignee));

        let mut issue = issues.get_mut(&id).unwrap();
        issue
            .assign([assignee, assignee_two], &node.signer)
            .unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let assignees: Vec<_> = issue.assigned().cloned().collect::<Vec<_>>();

        assert_eq!(2, assignees.len());
        assert!(assignees.contains(&assignee));
        assert!(assignees.contains(&assignee_two));
    }

    #[test]
    fn test_issue_create_and_reassign() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();

        let assignee = Did::from(arbitrary::gen::<ActorId>(1));
        let assignee_two = Did::from(arbitrary::gen::<ActorId>(1));
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[assignee],
                [],
                &node.signer,
            )
            .unwrap();

        issue.assign([assignee_two], &node.signer).unwrap();
        issue.assign([assignee_two], &node.signer).unwrap();
        issue.reload().unwrap();

        let assignees: Vec<_> = issue.assigned().cloned().collect::<Vec<_>>();

        assert_eq!(1, assignees.len());
        assert!(assignees.contains(&assignee_two));

        issue.assign([], &node.signer).unwrap();
        issue.reload().unwrap();

        assert_eq!(0, issue.assigned().count());
    }

    #[test]
    fn test_issue_create_and_get() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let created = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        let (id, created) = (created.id, created.issue);
        let issue = issues.get(&id).unwrap().unwrap();

        assert_eq!(created, issue);
        assert_eq!(issue.title(), "My first issue");
        assert_eq!(issue.author().id, Did::from(node.signer.public_key()));
        assert_eq!(issue.description().1, "Blah blah blah.");
        assert_eq!(issue.comments().count(), 1);
        assert_eq!(issue.state(), &State::Open);
    }

    #[test]
    fn test_issue_create_and_change_state() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        issue
            .lifecycle(
                State::Closed {
                    reason: CloseReason::Other,
                },
                &node.signer,
            )
            .unwrap();

        let id = issue.id;
        let mut issue = issues.get_mut(&id).unwrap();

        assert_eq!(
            *issue.state(),
            State::Closed {
                reason: CloseReason::Other
            }
        );

        issue.lifecycle(State::Open, &node.signer).unwrap();
        let issue = issues.get(&id).unwrap().unwrap();

        assert_eq!(*issue.state(), State::Open);
    }

    #[test]
    fn test_issue_create_and_unassign() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();

        let assignee = Did::from(arbitrary::gen::<ActorId>(1));
        let assignee_two = Did::from(arbitrary::gen::<ActorId>(1));
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[assignee, assignee_two],
                [],
                &node.signer,
            )
            .unwrap();
        assert_eq!(2, issue.assigned().count());

        issue.assign([assignee_two], &node.signer).unwrap();
        issue.reload().unwrap();

        let assignees: Vec<_> = issue.assigned().cloned().collect::<Vec<_>>();

        assert_eq!(1, assignees.len());
        assert!(assignees.contains(&assignee_two));
    }

    #[test]
    fn test_issue_edit() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        issue.edit("Sorry typo", &node.signer).unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let r = issue.title();

        assert_eq!(r, "Sorry typo");
    }

    #[test]
    fn test_issue_edit_description() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        issue
            .edit_description("Bob Loblaw law blog", vec![], &node.signer)
            .unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let (_, desc) = issue.description();

        assert_eq!(desc, "Bob Loblaw law blog");
    }

    #[test]
    fn test_issue_react() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        let (comment, _) = issue.root();
        let comment = *comment;
        let reaction = Reaction::new('🥳').unwrap();
        issue.react(comment, reaction, true, &node.signer).unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let (_, r) = issue.comment(&comment).unwrap().reactions().next().unwrap();

        assert_eq!(r, &reaction);

        // TODO: Test multiple reactions from same author and different authors
    }

    #[test]
    fn test_issue_reply() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();
        let (root, _) = issue.root();
        let root = *root;

        let c1 = issue
            .comment("Hi hi hi.", root, vec![], &node.signer)
            .unwrap();
        let c2 = issue
            .comment("Ha ha ha.", root, vec![], &node.signer)
            .unwrap();

        let id = issue.id;
        let mut issue = issues.get_mut(&id).unwrap();
        let (_, reply1) = &issue.replies(&root).nth(0).unwrap();
        let (_, reply2) = &issue.replies(&root).nth(1).unwrap();

        assert_eq!(reply1.body(), "Hi hi hi.");
        assert_eq!(reply2.body(), "Ha ha ha.");

        issue.comment("Re: Hi.", c1, vec![], &node.signer).unwrap();
        issue.comment("Re: Ha.", c2, vec![], &node.signer).unwrap();
        issue
            .comment("Re: Ha. Ha.", c2, vec![], &node.signer)
            .unwrap();
        issue
            .comment("Re: Ha. Ha. Ha.", c2, vec![], &node.signer)
            .unwrap();

        let issue = issues.get(&id).unwrap().unwrap();

        assert_eq!(issue.replies(&c1).nth(0).unwrap().1.body(), "Re: Hi.");
        assert_eq!(issue.replies(&c2).nth(0).unwrap().1.body(), "Re: Ha.");
        assert_eq!(issue.replies(&c2).nth(1).unwrap().1.body(), "Re: Ha. Ha.");
        assert_eq!(
            issue.replies(&c2).nth(2).unwrap().1.body(),
            "Re: Ha. Ha. Ha."
        );
    }

    #[test]
    fn test_issue_label() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let bug_label = Label::new("bug").unwrap();
        let ux_label = Label::new("ux").unwrap();
        let wontfix_label = Label::new("wontfix").unwrap();
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[ux_label.clone()],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        issue
            .label([ux_label.clone(), bug_label.clone()], &node.signer)
            .unwrap();
        issue
            .label(
                [ux_label.clone(), bug_label.clone(), wontfix_label.clone()],
                &node.signer,
            )
            .unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let labels = issue.labels().cloned().collect::<Vec<_>>();

        assert!(labels.contains(&ux_label));
        assert!(labels.contains(&bug_label));
        assert!(labels.contains(&wontfix_label));
    }

    #[test]
    fn test_issue_comment() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let author = *node.signer.public_key();
        let mut issues = Issues::open(&*repo).unwrap();
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        // The root thread op id is always the same.
        let (c0, _) = issue.root();
        let c0 = *c0;

        issue
            .comment("Ho ho ho.", c0, vec![], &node.signer)
            .unwrap();
        issue
            .comment("Ha ha ha.", c0, vec![], &node.signer)
            .unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let (_, c0) = &issue.comments().nth(0).unwrap();
        let (_, c1) = &issue.comments().nth(1).unwrap();
        let (_, c2) = &issue.comments().nth(2).unwrap();

        assert_eq!(c0.body(), "Blah blah blah.");
        assert_eq!(c0.author(), author);
        assert_eq!(c1.body(), "Ho ho ho.");
        assert_eq!(c1.author(), author);
        assert_eq!(c2.body(), "Ha ha ha.");
        assert_eq!(c2.author(), author);
    }

    #[test]
    fn test_issue_state_serde() {
        assert_eq!(
            serde_json::to_value(State::Open).unwrap(),
            serde_json::json!({ "status": "open" })
        );

        assert_eq!(
            serde_json::to_value(State::Closed {
                reason: CloseReason::Solved
            })
            .unwrap(),
            serde_json::json!({ "status": "closed", "reason": "solved" })
        );
    }

    #[test]
    fn test_issue_all() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();

        issues
            .create("First", "Blah", &[], &[], [], &node.signer)
            .unwrap();
        issues
            .create("Second", "Blah", &[], &[], [], &node.signer)
            .unwrap();
        issues
            .create("Third", "Blah", &[], &[], [], &node.signer)
            .unwrap();

        let issues = issues
            .all()
            .unwrap()
            .map(|r| r.map(|(_, i)| i))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(issues.len(), 3);

        issues.iter().find(|i| i.title() == "First").unwrap();
        issues.iter().find(|i| i.title() == "Second").unwrap();
        issues.iter().find(|i| i.title() == "Third").unwrap();
    }

    #[test]
    fn test_issue_multilines() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let created = issues
            .create(
                "My first issue",
                "Blah blah blah.\nYah yah yah",
                &[],
                &[],
                [],
                &node.signer,
            )
            .unwrap();

        let (id, created) = (created.id, created.issue);
        let issue = issues.get(&id).unwrap().unwrap();

        assert_eq!(created, issue);
        assert_eq!(issue.title(), "My first issue");
        assert_eq!(issue.author().id, Did::from(node.signer.public_key()));
        assert_eq!(issue.description().1, "Blah blah blah.\nYah yah yah");
        assert_eq!(issue.comments().count(), 1);
        assert_eq!(issue.state(), &State::Open);
    }

    #[test]
    fn test_embeds() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let embed1 = Embed {
            name: String::from("example.html"),
            content: b"<html>Hello World!</html>".to_vec(),
        };
        let embed2 = Embed {
            name: String::from("style.css"),
            content: b"body { color: red }".to_vec(),
        };
        let embed3 = Embed {
            name: String::from("bin"),
            content: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        };
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [embed1.clone(), embed2.clone()],
                &node.signer,
            )
            .unwrap();

        issue
            .comment(
                "Here's a binary file",
                issue.id.into(),
                [embed3.clone()],
                &node.signer,
            )
            .unwrap();

        issue.reload().unwrap();

        let (_, c0) = issue.thread().comments().next().unwrap();
        let (_, c1) = issue.thread().comments().next_back().unwrap();

        let e1 = &c0.embeds()[0];
        let e2 = &c0.embeds()[1];
        let e3 = &c1.embeds()[0];

        let b1 = repo.blob(Oid::try_from(&e1.content).unwrap()).unwrap();
        let b2 = repo.blob(Oid::try_from(&e2.content).unwrap()).unwrap();
        let b3 = repo.blob(Oid::try_from(&e3.content).unwrap()).unwrap();

        assert_eq!(b1.content(), &embed1.content);
        assert_eq!(b2.content(), &embed2.content);
        assert_eq!(b3.content(), &embed3.content);

        assert_eq!(b1.is_binary(), false);
        assert_eq!(b2.is_binary(), false);
        assert_eq!(b3.is_binary(), true);
    }

    #[test]
    fn test_embeds_edit() {
        let test::setup::NodeWithRepo { node, repo, .. } = test::setup::NodeWithRepo::default();
        let mut issues = Issues::open(&*repo).unwrap();
        let embed1 = Embed {
            name: String::from("example.html"),
            content: b"<html>Hello World!</html>".to_vec(),
        };
        let embed1_edited = Embed {
            name: String::from("example.html"),
            content: b"<html>Hello Radicle!</html>".to_vec(),
        };
        let embed2 = Embed {
            name: String::from("style.css"),
            content: b"body { color: red }".to_vec(),
        };
        let mut issue = issues
            .create(
                "My first issue",
                "Blah blah blah.",
                &[],
                &[],
                [embed1.clone(), embed2.clone()],
                &node.signer,
            )
            .unwrap();

        issue.reload().unwrap();
        issue
            .edit_description("My first issue", [embed1_edited.clone()], &node.signer)
            .unwrap();
        issue.reload().unwrap();

        let (_, c0) = issue.thread().comments().next().unwrap();

        assert_eq!(c0.embeds().len(), 1);

        let e1 = &c0.embeds()[0];
        let b1 = repo.blob(Oid::try_from(&e1.content).unwrap()).unwrap();

        assert_eq!(e1.content, Uri::from(embed1_edited.oid()));
        assert_eq!(b1.content(), &embed1_edited.content);
    }
}
