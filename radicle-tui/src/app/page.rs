use std::collections::HashMap;

use anyhow::Result;

use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::patch::{Patch, PatchId};

use radicle::cob::thread::{Comment, CommentId};
use radicle_tui::cob;
use radicle_tui::ui::widget::common::context::Shortcuts;
use tuirealm::{Frame, NoUserEvent, Sub, SubClause};

use radicle_tui::ui::context::Context;
use radicle_tui::ui::layout;
use radicle_tui::ui::theme::Theme;
use radicle_tui::ui::widget::{self, Widget};

use super::{
    subscription, Application, Cid, CommentCid, CommentMessage, HomeCid, IssueCid, IssueMessage,
    Message, PatchCid, PopupCid, WarningMessage,
};

/// `tuirealm`'s event and prop system is designed to work with flat component hierarchies.
/// Building deep nested component hierarchies would need a lot more additional effort to
/// properly pass events and props down these hierarchies. This makes it hard to implement
/// full app views (home, patch details etc) as components.
///
/// View pages take into account these flat component hierarchies, and provide
/// switchable sets of components.
pub trait ViewPage {
    /// Will be called whenever a view page is pushed onto the page stack. Should create and mount all widgets.
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()>;

    /// Will be called whenever a view page is popped from the page stack. Should unmount all widgets.
    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()>;

    /// Will be called whenever a view page is on top of the stack and can be used to update its internal
    /// state depending on the message passed.
    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<()>;

    /// Will be called whenever a view page is on top of the page stack and needs to be rendered.
    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame);

    /// Will be called whenever this view page is pushed to the stack, or it is on top of the stack again
    /// after another view page was popped from the stack.
    fn subscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()>;

    /// Will be called whenever this view page is on top of the stack and another view page is pushed
    /// to the stack, or if this is popped from the stack.
    fn unsubscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()>;
}

///
/// Home
///
pub struct HomeView {
    active_component: Cid,
}

impl Default for HomeView {
    fn default() -> Self {
        HomeView {
            active_component: Cid::Home(HomeCid::Dashboard),
        }
    }
}

impl ViewPage for HomeView {
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let navigation = widget::home::navigation(theme).to_boxed();

        let dashboard = widget::home::dashboard(context, theme).to_boxed();
        let issue_browser = widget::home::issues(context, theme).to_boxed();
        let patch_browser = widget::home::patches(context, theme).to_boxed();

        app.remount(Cid::Home(HomeCid::Navigation), navigation, vec![])?;

        app.remount(Cid::Home(HomeCid::Dashboard), dashboard, vec![])?;
        app.remount(Cid::Home(HomeCid::IssueBrowser), issue_browser, vec![])?;
        app.remount(Cid::Home(HomeCid::PatchBrowser), patch_browser, vec![])?;

        app.active(&self.active_component)?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Home(HomeCid::Navigation))?;
        app.umount(&Cid::Home(HomeCid::Dashboard))?;
        app.umount(&Cid::Home(HomeCid::IssueBrowser))?;
        app.umount(&Cid::Home(HomeCid::PatchBrowser))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        _context: &Context,
        _theme: &Theme,
        message: Message,
    ) -> Result<()> {
        if let Message::NavigationChanged(index) = message {
            self.active_component = Cid::Home(HomeCid::from(index as usize));
            app.active(&self.active_component)?;
        }

        Ok(())
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let layout = layout::default_page(area);

        app.view(&Cid::Home(HomeCid::Navigation), frame, layout[0]);
        app.view(&self.active_component, frame, layout[1]);
    }

    fn subscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.subscribe(
            &Cid::Home(HomeCid::Navigation),
            Sub::new(subscription::navigation_clause(), SubClause::Always),
        )?;

        Ok(())
    }

    fn unsubscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.unsubscribe(
            &Cid::Home(HomeCid::Navigation),
            subscription::navigation_clause(),
        )?;

        Ok(())
    }
}

///
/// Issue detail page
///
pub struct IssuePage {
    issue: (IssueId, Issue),
}

impl IssuePage {
    pub fn new(issue: (IssueId, Issue)) -> Self {
        IssuePage { issue }
    }
}

impl ViewPage for IssuePage {
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let (id, issue) = &self.issue;
        let list = widget::issue::list(context, theme, (*id, issue.clone())).to_boxed();
        let details = widget::issue::details(context, theme, (*id, issue.clone())).to_boxed();
        let discussion =
            widget::issue::issue_discussion(context, theme, (*id, issue.clone())).to_boxed();
        let shortcuts = widget::common::shortcuts(
            theme,
            vec![
                widget::common::shortcut(theme, "esc", "back"),
                widget::common::shortcut(theme, "q", "quit"),
            ],
        )
        .to_boxed();

        app.remount(Cid::Issue(IssueCid::List), list, vec![])?;
        app.remount(Cid::Issue(IssueCid::Details), details, vec![])?;
        app.remount(Cid::Issue(IssueCid::Discussion), discussion, vec![])?;
        app.remount(Cid::Issue(IssueCid::Shortcuts), shortcuts, vec![])?;

        app.active(&Cid::Issue(IssueCid::List))?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Issue(IssueCid::List))?;
        app.umount(&Cid::Issue(IssueCid::Details))?;
        app.umount(&Cid::Issue(IssueCid::Discussion))?;
        app.umount(&Cid::Issue(IssueCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<()> {
        match message {
            Message::Issue(IssueMessage::Changed(id)) => {
                let repo = context.repository();
                if let Some(issue) = cob::issue::find(repo, &id)? {
                    let details =
                        widget::issue::details(context, theme, (id, issue.clone())).to_boxed();
                    app.remount(Cid::Issue(IssueCid::Details), details, vec![])?;

                    let discussion =
                        widget::issue::issue_discussion(context, theme, (id, issue)).to_boxed();
                    app.remount(Cid::Issue(IssueCid::Discussion), discussion, vec![])?;
                }
            }
            Message::Issue(IssueMessage::Focus(cid)) => {
                app.active(&Cid::Issue(cid))?;
            }
            _ => {}
        }

        Ok(())
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let shortcuts_h = 1u16;
        let layout = layout::issue_page(area, shortcuts_h);

        app.view(&Cid::Issue(IssueCid::List), frame, layout.list);
        app.view(&Cid::Issue(IssueCid::Details), frame, layout.details);
        app.view(&Cid::Issue(IssueCid::Discussion), frame, layout.discussion);
        app.view(&Cid::Issue(IssueCid::Shortcuts), frame, layout.shortcuts);
    }

    fn subscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }

    fn unsubscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }
}

///
/// Discussion detail page
///
pub struct CommentPage {
    issue: (IssueId, Issue),
    comment: (CommentId, Comment),
    shortcuts: HashMap<CommentCid, Widget<Shortcuts>>,
}

impl CommentPage {
    pub fn new(theme: Theme, issue: (IssueId, Issue), comment: (CommentId, Comment)) -> Self {
        let shortcuts = Self::build_shortcuts(&theme);

        Self {
            issue,
            comment,
            shortcuts,
        }
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<CommentCid, Widget<Shortcuts>> {
        [
            (
                CommentCid::Discussion,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "esc", "back"),
                        widget::common::shortcut(theme, "↑/↓", "navigate"),
                        widget::common::shortcut(theme, "enter", "view"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                CommentCid::Body,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "esc", "back"),
                        widget::common::shortcut(theme, "↑/↓", "scroll"),
                        widget::common::shortcut(theme, "c", "reply"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
        ]
        .iter()
        .cloned()
        .collect()
    }
}

impl ViewPage for CommentPage {
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let details = widget::issue::details(context, theme, self.issue.clone()).to_boxed();
        let discussion = widget::issue::comment_discussion(
            context,
            theme,
            self.issue.clone(),
            Some(self.comment.clone()),
        )
        .to_boxed();
        let body = widget::issue::comment_body(context, theme, self.comment.clone()).to_boxed();
        let editor = widget::issue::comment_reply(context, theme, self.comment.clone()).to_boxed();

        app.remount(Cid::Comment(CommentCid::Details), details, vec![])?;
        app.remount(Cid::Comment(CommentCid::Discussion), discussion, vec![])?;
        app.remount(Cid::Comment(CommentCid::Body), body, vec![])?;
        app.remount(Cid::Comment(CommentCid::Editor), editor, vec![])?;

        let active_cid = CommentCid::Discussion;
        if let Some(shortcuts) = self.shortcuts.get(&active_cid) {
            app.remount(
                Cid::Comment(CommentCid::Shortcuts),
                shortcuts.clone().to_boxed(),
                vec![],
            )?;
        }

        app.active(&Cid::Comment(active_cid))?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Comment(CommentCid::Details))?;
        app.umount(&Cid::Comment(CommentCid::Discussion))?;
        app.umount(&Cid::Comment(CommentCid::Body))?;
        app.umount(&Cid::Comment(CommentCid::Editor))?;
        app.umount(&Cid::Comment(CommentCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<()> {
        match message {
            Message::Comment(CommentMessage::Focus(cid)) => {
                if let Some(shortcuts) = self.shortcuts.get(&cid) {
                    app.remount(
                        Cid::Comment(CommentCid::Shortcuts),
                        shortcuts.clone().to_boxed(),
                        vec![],
                    )?;
                }

                app.active(&Cid::Comment(cid))?;
            }
            Message::Comment(CommentMessage::Changed(id)) => {
                let (_, issue) = self.issue.clone();
                let comment = issue.comments().find(|(comment_id, _)| *comment_id == &id);

                if let Some((comment_id, comment)) = comment {
                    let body =
                        widget::issue::comment_body(context, theme, (*comment_id, comment.clone()));
                    app.remount(Cid::Comment(CommentCid::Body), body.to_boxed(), vec![])?;
                }
            }
            Message::Warning(WarningMessage::Show(message)) => {
                let popup = widget::common::warning(theme, message).to_boxed();
                app.remount(Cid::Popup(PopupCid::Warning), popup, vec![])?;

                app.active(&Cid::Popup(PopupCid::Warning))?;
            }
            Message::Warning(WarningMessage::Hide) => {
                app.blur()?;
                app.umount(&Cid::Popup(PopupCid::Warning))?;
            }
            _ => {}
        }

        Ok(())
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let shortcuts_h = 1u16;
        let layout = layout::comment_page(area, shortcuts_h);

        app.view(&Cid::Comment(CommentCid::Details), frame, layout.details);
        app.view(
            &Cid::Comment(CommentCid::Discussion),
            frame,
            layout.discussion,
        );
        app.view(&Cid::Comment(CommentCid::Body), frame, layout.body);
        app.view(&Cid::Comment(CommentCid::Editor), frame, layout.editor);
        app.view(
            &Cid::Comment(CommentCid::Shortcuts),
            frame,
            layout.shortcuts,
        );

        if app.mounted(&Cid::Popup(PopupCid::Warning)) {
            app.view(&Cid::Popup(PopupCid::Warning), frame, area);
        }
    }

    fn subscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }

    fn unsubscribe(&self, _app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        Ok(())
    }
}

///
/// Patch detail page
///
pub struct PatchView {
    active_component: Cid,
    patch: (PatchId, Patch),
}

impl PatchView {
    pub fn new(patch: (PatchId, Patch)) -> Self {
        PatchView {
            active_component: Cid::Patch(PatchCid::Activity),
            patch,
        }
    }
}

impl ViewPage for PatchView {
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let (id, patch) = &self.patch;
        let navigation = widget::patch::navigation(theme).to_boxed();
        let activity = widget::patch::activity(theme, (*id, patch), context.profile()).to_boxed();
        let files = widget::patch::files(theme, (*id, patch), context.profile()).to_boxed();

        app.remount(Cid::Patch(PatchCid::Navigation), navigation, vec![])?;
        app.remount(Cid::Patch(PatchCid::Activity), activity, vec![])?;
        app.remount(Cid::Patch(PatchCid::Files), files, vec![])?;

        app.active(&self.active_component)?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Patch(PatchCid::Navigation))?;
        app.umount(&Cid::Patch(PatchCid::Activity))?;
        app.umount(&Cid::Patch(PatchCid::Files))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        _context: &Context,
        _theme: &Theme,
        message: Message,
    ) -> Result<()> {
        if let Message::NavigationChanged(index) = message {
            self.active_component = Cid::Patch(PatchCid::from(index as usize));
        }
        app.active(&self.active_component)?;

        Ok(())
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let layout = layout::default_page(area);

        app.view(&Cid::Patch(PatchCid::Navigation), frame, layout[0]);
        app.view(&self.active_component, frame, layout[1]);
    }

    fn subscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.subscribe(
            &Cid::Patch(PatchCid::Navigation),
            Sub::new(subscription::navigation_clause(), SubClause::Always),
        )?;

        Ok(())
    }

    fn unsubscribe(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.unsubscribe(
            &Cid::Patch(PatchCid::Navigation),
            subscription::navigation_clause(),
        )?;

        Ok(())
    }
}

/// View pages need to preserve their state (e.g. selected navigation tab, contents
/// and the selected row of a table). Therefor they should not be (re-)created
/// each time they are displayed.
/// Instead the application can push a new page onto the page stack if it needs to
/// be displayed. Its components are then created using the internal state. If a
/// new page needs to be displayed, it will also be pushed onto the stack. Leaving
/// that page again will pop it from the stack. The application can then return to
/// the previously displayed page in the state it was left.
#[derive(Default)]
pub struct PageStack {
    pages: Vec<Box<dyn ViewPage>>,
}

impl PageStack {
    pub fn push(
        &mut self,
        page: Box<dyn ViewPage>,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        if let Some(page) = self.pages.last() {
            page.unsubscribe(app)?;
        }

        page.mount(app, context, theme)?;
        page.subscribe(app)?;

        self.pages.push(page);

        Ok(())
    }

    pub fn pop(&mut self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        self.peek_mut()?.unsubscribe(app)?;
        self.peek_mut()?.unmount(app)?;
        self.pages.pop();

        self.peek_mut()?.subscribe(app)?;

        Ok(())
    }

    pub fn peek_mut(&mut self) -> Result<&mut Box<dyn ViewPage>> {
        match self.pages.last_mut() {
            Some(page) => Ok(page),
            None => Err(anyhow::anyhow!(
                "Could not peek active page. Page stack is empty."
            )),
        }
    }
}

impl From<usize> for HomeCid {
    fn from(index: usize) -> Self {
        match index {
            0 => HomeCid::Dashboard,
            1 => HomeCid::IssueBrowser,
            2 => HomeCid::PatchBrowser,
            _ => HomeCid::Dashboard,
        }
    }
}

impl From<usize> for PatchCid {
    fn from(index: usize) -> Self {
        match index {
            0 => PatchCid::Activity,
            1 => PatchCid::Files,
            _ => PatchCid::Activity,
        }
    }
}
