use std::ops::{ControlFlow, Deref};
use std::str::FromStr;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use radicle_crdt::clock;
use radicle_crdt::{LWWReg, LWWSet, Max, Semilattice};

use crate::cob;
use crate::cob::common::{Author, Reaction, Tag};
use crate::cob::store::Transaction;
use crate::cob::thread;
use crate::cob::thread::{CommentId, Thread};
use crate::cob::{store, ObjectId, OpId, TypeName};
use crate::crypto::{PublicKey, Signer};
use crate::storage::git as storage;

use super::op::Ops;

/// Issue operation.
pub type Op = crate::cob::Op<Action>;

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
            Self::Open { .. } => write!(f, "open"),
        }
    }
}

impl State {
    pub fn lifecycle_message(self) -> String {
        match self {
            State::Open => "Open issue".to_owned(),
            State::Closed { .. } => "Close issue".to_owned(),
        }
    }
}

/// Issue state. Accumulates [`Action`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    title: LWWReg<Max<String>, clock::Lamport>,
    state: LWWReg<Max<State>, clock::Lamport>,
    tags: LWWSet<Tag>,
    thread: Thread,
}

impl Semilattice for Issue {
    fn merge(&mut self, other: Self) {
        self.title.merge(other.title);
        self.state.merge(other.state);
        self.thread.merge(other.thread);
    }
}

impl Default for Issue {
    fn default() -> Self {
        Self {
            title: Max::from(String::default()).into(),
            state: Max::from(State::default()).into(),
            tags: LWWSet::default(),
            thread: Thread::default(),
        }
    }
}

impl store::FromHistory for Issue {
    type Action = Action;

    fn type_name() -> &'static TypeName {
        &*TYPENAME
    }

    fn from_history(
        history: &radicle_cob::History,
    ) -> Result<(Self, clock::Lamport), store::Error> {
        let obj = history.traverse(Self::default(), |mut acc, entry| {
            if let Ok(Ops(ops)) = Ops::try_from(entry) {
                if let Err(err) = acc.apply(ops) {
                    log::warn!("Error applying op to issue state: {err}");
                    return ControlFlow::Break(acc);
                }
            } else {
                return ControlFlow::Break(acc);
            }
            ControlFlow::Continue(acc)
        });

        Ok((obj, history.clock().into()))
    }
}

impl Issue {
    pub fn title(&self) -> &str {
        self.title.get().as_str()
    }

    pub fn state(&self) -> &State {
        self.state.get()
    }

    pub fn tags(&self) -> impl Iterator<Item = &Tag> {
        self.tags.iter()
    }

    pub fn author(&self) -> Option<Author> {
        self.thread
            .comments()
            .next()
            .map(|((_, pk), _)| Author::new(*pk))
    }

    pub fn description(&self) -> Option<&str> {
        self.thread.comments().next().map(|(_, c)| c.body.as_str())
    }

    pub fn comments(&self) -> impl Iterator<Item = (&CommentId, &thread::Comment)> {
        self.thread.comments()
    }

    pub fn apply(&mut self, ops: impl IntoIterator<Item = Op>) -> Result<(), Error> {
        for op in ops {
            match op.action {
                Action::Edit { title } => {
                    self.title.set(title, op.clock);
                }
                Action::Lifecycle { state } => {
                    self.state.set(state, op.clock);
                }
                Action::Tag { add, remove } => {
                    for tag in add {
                        self.tags.insert(tag, op.clock);
                    }
                    for tag in remove {
                        self.tags.remove(tag, op.clock);
                    }
                }
                Action::Thread { action } => {
                    self.thread.apply([cob::Op {
                        action,
                        author: op.author,
                        clock: op.clock,
                        timestamp: op.timestamp,
                    }]);
                }
            }
        }
        Ok(())
    }
}

impl Deref for Issue {
    type Target = Thread;

    fn deref(&self) -> &Self::Target {
        &self.thread
    }
}

impl store::Transaction<Issue> {
    /// Set the issue title.
    pub fn edit(&mut self, title: impl ToString) -> OpId {
        self.push(Action::Edit {
            title: title.to_string(),
        })
    }

    /// Lifecycle an issue.
    pub fn lifecycle(&mut self, state: State) -> OpId {
        self.push(Action::Lifecycle { state })
    }

    /// Comment on an issue.
    pub fn comment<S: ToString>(&mut self, body: S) -> CommentId {
        self.push(Action::from(thread::Action::Comment {
            body: body.to_string(),
            reply_to: None,
        }))
    }

    /// Tag an issue.
    pub fn tag(
        &mut self,
        add: impl IntoIterator<Item = Tag>,
        remove: impl IntoIterator<Item = Tag>,
    ) -> OpId {
        let add = add.into_iter().collect::<Vec<_>>();
        let remove = remove.into_iter().collect::<Vec<_>>();

        self.push(Action::Tag { add, remove })
    }

    /// Reply to on an issue comment.
    pub fn reply<S: ToString>(&mut self, parent: CommentId, body: S) -> OpId {
        let body = body.to_string();

        self.push(Action::from(thread::Action::Comment {
            body,
            reply_to: Some(parent),
        }))
    }

    /// React to an issue comment.
    pub fn react(&mut self, to: CommentId, reaction: Reaction) -> OpId {
        self.push(Action::Thread {
            action: thread::Action::React {
                to,
                reaction,
                active: true,
            },
        })
    }
}

pub struct IssueMut<'a, 'g> {
    id: ObjectId,
    clock: clock::Lamport,
    issue: Issue,
    store: &'g mut Issues<'a>,
}

impl<'a, 'g> IssueMut<'a, 'g> {
    /// Get the internal logical clock.
    pub fn clock(&self) -> &clock::Lamport {
        &self.clock
    }

    /// Lifecycle an issue.
    pub fn lifecycle<G: Signer>(&mut self, state: State, signer: &G) -> Result<OpId, Error> {
        self.transaction("Lifecycle", signer, |tx| tx.lifecycle(state))
    }

    /// Comment on an issue.
    pub fn comment<G: Signer, S: ToString>(
        &mut self,
        body: S,
        signer: &G,
    ) -> Result<CommentId, Error> {
        self.transaction("Comment", signer, |tx| tx.comment(body))
    }

    /// Tag an issue.
    pub fn tag<G: Signer>(
        &mut self,
        add: impl IntoIterator<Item = Tag>,
        remove: impl IntoIterator<Item = Tag>,
        signer: &G,
    ) -> Result<OpId, Error> {
        self.transaction("Tag", signer, |tx| tx.tag(add, remove))
    }

    /// Reply to on an issue comment.
    pub fn reply<G: Signer, S: ToString>(
        &mut self,
        parent: CommentId,
        body: S,
        signer: &G,
    ) -> Result<OpId, Error> {
        assert!(self.thread.comment(&parent).is_some());
        self.transaction("Reply", signer, |tx| tx.reply(parent, body))
    }

    /// React to an issue comment.
    pub fn react<G: Signer>(
        &mut self,
        to: CommentId,
        reaction: Reaction,
        signer: &G,
    ) -> Result<OpId, Error> {
        self.transaction("React", signer, |tx| tx.react(to, reaction))
    }

    pub fn transaction<G, F, T>(
        &mut self,
        message: &str,
        signer: &G,
        operations: F,
    ) -> Result<T, Error>
    where
        G: Signer,
        F: FnOnce(&mut Transaction<Issue>) -> T,
    {
        let mut tx = Transaction::new(*signer.public_key(), self.clock);
        let output = operations(&mut tx);
        let (ops, clock) = tx.commit(message, self.id, &mut self.store.raw, signer)?;

        self.issue.apply(ops)?;
        self.clock = clock;

        Ok(output)
    }
}

impl<'a, 'g> Deref for IssueMut<'a, 'g> {
    type Target = Issue;

    fn deref(&self) -> &Self::Target {
        &self.issue
    }
}

pub struct Issues<'a> {
    raw: store::Store<'a, Issue>,
}

impl<'a> Deref for Issues<'a> {
    type Target = store::Store<'a, Issue>;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<'a> Issues<'a> {
    /// Open an issues store.
    pub fn open(
        whoami: PublicKey,
        repository: &'a storage::Repository,
    ) -> Result<Self, store::Error> {
        let raw = store::Store::open(whoami, repository)?;

        Ok(Self { raw })
    }

    /// Get an issue.
    pub fn get(&self, id: &ObjectId) -> Result<Option<Issue>, store::Error> {
        self.raw.get(id).map(|r| r.map(|(i, _clock)| i))
    }

    /// Get an issue mutably.
    pub fn get_mut<'g>(&'g mut self, id: &ObjectId) -> Result<IssueMut<'a, 'g>, store::Error> {
        let (issue, clock) = self
            .raw
            .get(id)?
            .ok_or_else(move || store::Error::NotFound(TYPENAME.clone(), *id))?;

        Ok(IssueMut {
            id: *id,
            clock,
            issue,
            store: self,
        })
    }

    /// Create a new issue.
    pub fn create<'g, G: Signer>(
        &'g mut self,
        title: impl ToString,
        description: impl ToString,
        tags: &[Tag],
        signer: &G,
    ) -> Result<IssueMut<'a, 'g>, Error> {
        let (id, issue, clock) =
            Transaction::initial("Create issue", &mut self.raw, signer, |tx| {
                tx.edit(title);
                tx.comment(description);
                tx.tag(tags.to_owned(), []);
            })?;
        // Just a sanity check that our clock is advancing as expected.
        assert_eq!(clock.get(), 2);

        Ok(IssueMut {
            id,
            clock,
            issue,
            store: self,
        })
    }

    /// Remove an issue.
    pub fn remove(&self, id: &ObjectId) -> Result<(), store::Error> {
        self.raw.remove(id)
    }
}

/// Issue operation.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Edit { title: String },
    Lifecycle { state: State },
    Tag { add: Vec<Tag>, remove: Vec<Tag> },
    Thread { action: thread::Action },
}

impl From<thread::Action> for Action {
    fn from(action: thread::Action) -> Self {
        Self::Thread { action }
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::cob::Reaction;
    use crate::test;

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
    fn test_issue_create_and_get() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, signer, project) = test::setup::context(&tmp);
        let mut issues = Issues::open(*signer.public_key(), &project).unwrap();
        let created = issues
            .create("My first issue", "Blah blah blah.", &[], &signer)
            .unwrap();
        let (id, created) = (created.id, created.issue);
        let issue = issues.get(&id).unwrap().unwrap();

        assert_eq!(created, issue);
        assert_eq!(issue.title(), "My first issue");
        assert_eq!(issue.author(), Some(issues.author()));
        assert_eq!(issue.description(), Some("Blah blah blah."));
        assert_eq!(issue.comments().count(), 1);
        assert_eq!(issue.state(), &State::Open);
    }

    #[test]
    fn test_issue_create_and_change_state() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, signer, project) = test::setup::context(&tmp);
        let mut issues = Issues::open(*signer.public_key(), &project).unwrap();
        let mut issue = issues
            .create("My first issue", "Blah blah blah.", &[], &signer)
            .unwrap();

        issue
            .lifecycle(
                State::Closed {
                    reason: CloseReason::Other,
                },
                &signer,
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

        issue.lifecycle(State::Open, &signer).unwrap();
        let issue = issues.get(&id).unwrap().unwrap();
        assert_eq!(*issue.state(), State::Open);
    }

    #[test]
    fn test_issue_react() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, signer, project) = test::setup::context(&tmp);
        let mut issues = Issues::open(*signer.public_key(), &project).unwrap();
        let mut issue = issues
            .create("My first issue", "Blah blah blah.", &[], &signer)
            .unwrap();

        let comment = (clock::Lamport::default(), *signer.public_key());
        let reaction = Reaction::new('🥳').unwrap();
        issue.react(comment, reaction, &signer).unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let (_, r) = issue.reactions(&comment).next().unwrap();

        assert_eq!(r, &reaction);

        // TODO: Test multiple reactions from same author and different authors
    }

    #[test]
    fn test_issue_reply() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, signer, project) = test::setup::context(&tmp);
        let mut issues = Issues::open(*signer.public_key(), &project).unwrap();
        let mut issue = issues
            .create("My first issue", "Blah blah blah.", &[], &signer)
            .unwrap();
        let comment = issue.comment("Ho ho ho.", &signer).unwrap();

        issue.reply(comment, "Hi hi hi.", &signer).unwrap();
        issue.reply(comment, "Ha ha ha.", &signer).unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let (_, reply1) = &issue.replies(&comment).nth(0).unwrap();
        let (_, reply2) = &issue.replies(&comment).nth(1).unwrap();

        assert_eq!(reply1.body, "Hi hi hi.");
        assert_eq!(reply2.body, "Ha ha ha.");
    }

    #[test]
    fn test_issue_tag() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, signer, project) = test::setup::context(&tmp);
        let mut issues = Issues::open(*signer.public_key(), &project).unwrap();
        let mut issue = issues
            .create("My first issue", "Blah blah blah.", &[], &signer)
            .unwrap();

        let bug_tag = Tag::new("bug").unwrap();
        let wontfix_tag = Tag::new("wontfix").unwrap();

        issue.tag([bug_tag.clone()], [], &signer).unwrap();
        issue.tag([wontfix_tag.clone()], [], &signer).unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let tags = issue.tags().cloned().collect::<Vec<_>>();

        assert!(tags.contains(&bug_tag));
        assert!(tags.contains(&wontfix_tag));
    }

    #[test]
    fn test_issue_comment() {
        let tmp = tempfile::tempdir().unwrap();
        let (_, signer, project) = test::setup::context(&tmp);
        let author = *signer.public_key();
        let mut issues = Issues::open(*signer.public_key(), &project).unwrap();
        let mut issue = issues
            .create("My first issue", "Blah blah blah.", &[], &signer)
            .unwrap();

        issue.comment("Ho ho ho.", &signer).unwrap();
        issue.comment("Ha ha ha.", &signer).unwrap();

        let id = issue.id;
        let issue = issues.get(&id).unwrap().unwrap();
        let ((_, a0), c0) = &issue.comments().nth(0).unwrap();
        let ((_, a1), c1) = &issue.comments().nth(1).unwrap();
        let ((_, a2), c2) = &issue.comments().nth(2).unwrap();

        assert_eq!(&c0.body, "Blah blah blah.");
        assert_eq!(a0, &author);
        assert_eq!(&c1.body, "Ho ho ho.");
        assert_eq!(a1, &author);
        assert_eq!(&c2.body, "Ha ha ha.");
        assert_eq!(a2, &author);
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
        let tmp = tempfile::tempdir().unwrap();
        let (_, signer, project) = test::setup::context(&tmp);
        let mut issues = Issues::open(*signer.public_key(), &project).unwrap();

        issues.create("First", "Blah", &[], &signer).unwrap();
        issues.create("Second", "Blah", &[], &signer).unwrap();
        issues.create("Third", "Blah", &[], &signer).unwrap();

        let issues = issues
            .all()
            .unwrap()
            .map(|r| r.map(|(_, i, _)| i))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(issues.len(), 3);

        issues.iter().find(|i| i.title() == "First").unwrap();
        issues.iter().find(|i| i.title() == "Second").unwrap();
        issues.iter().find(|i| i.title() == "Third").unwrap();
    }
}
