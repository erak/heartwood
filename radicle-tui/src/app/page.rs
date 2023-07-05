use std::collections::HashMap;

use anyhow::Result;

use radicle::cob::issue::{Issue, IssueId};
use radicle::cob::patch::{Patch, PatchId};

use radicle_tui::cob;
use radicle_tui::ui::widget::common::context::{Progress, Shortcuts};
use tuirealm::{Frame, NoUserEvent, StateValue, Sub, SubClause};

use radicle_tui::ui::context::Context;
use radicle_tui::ui::layout;
use radicle_tui::ui::theme::Theme;
use radicle_tui::ui::widget::{self, Widget};

use super::{subscription, Application, Cid, HomeCid, IssueCid, IssueMessage, Message, PatchCid};

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
    ) -> Result<Option<Message>>;

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
    active_component: HomeCid,
    shortcuts: HashMap<HomeCid, Widget<Shortcuts>>,
}

impl HomeView {
    pub fn new(theme: Theme) -> Self {
        let shortcuts = Self::build_shortcuts(&theme);
        HomeView {
            active_component: HomeCid::Dashboard,
            shortcuts,
        }
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<HomeCid, Widget<Shortcuts>> {
        [
            (
                HomeCid::Dashboard,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "tab", "section"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                HomeCid::IssueBrowser,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "tab", "section"),
                        widget::common::shortcut(theme, "↑/↓", "navigate"),
                        widget::common::shortcut(theme, "enter", "show"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                HomeCid::PatchBrowser,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "tab", "section"),
                        widget::common::shortcut(theme, "↑/↓", "navigate"),
                        widget::common::shortcut(theme, "enter", "show"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
        ]
        .iter()
        .cloned()
        .collect()
    }

    fn update_context(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        cid: HomeCid,
    ) -> Result<()> {
        use tuirealm::State;

        let context = match cid {
            HomeCid::IssueBrowser => {
                let state = app.state(&Cid::Home(HomeCid::IssueBrowser))?;
                let progress = match state {
                    State::Tup2((StateValue::Usize(step), StateValue::Usize(total))) => {
                        Progress::Step(step.saturating_add(1), total)
                    }
                    _ => Progress::None,
                };
                let context = widget::issue::browse_context(context, theme, progress);
                Some(context)
            }
            HomeCid::PatchBrowser => {
                let state = app.state(&Cid::Home(HomeCid::PatchBrowser))?;
                let progress = match state {
                    State::Tup2((StateValue::Usize(step), StateValue::Usize(total))) => {
                        Progress::Step(step.saturating_add(1), total)
                    }
                    _ => Progress::None,
                };
                let context = widget::patch::browse_context(context, theme, progress);
                Some(context)
            }
            _ => None,
        };

        if let Some(context) = context {
            app.remount(Cid::Home(HomeCid::Context), context.to_boxed(), vec![])?;
        }

        Ok(())
    }

    fn update_shortcuts(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: HomeCid,
    ) -> Result<()> {
        if let Some(shortcuts) = self.shortcuts.get(&cid) {
            app.remount(
                Cid::Home(HomeCid::Shortcuts),
                shortcuts.clone().to_boxed(),
                vec![],
            )?;
        }
        Ok(())
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

        let active_component = Cid::Home(self.active_component.clone());
        app.active(&active_component)?;
        self.update_shortcuts(app, self.active_component.clone())?;
        self.update_context(app, context, theme, self.active_component.clone())?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Home(HomeCid::Navigation))?;
        app.umount(&Cid::Home(HomeCid::Dashboard))?;
        app.umount(&Cid::Home(HomeCid::IssueBrowser))?;
        app.umount(&Cid::Home(HomeCid::PatchBrowser))?;
        app.umount(&Cid::Home(HomeCid::Context))?;
        app.umount(&Cid::Home(HomeCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<Option<Message>> {
        if let Message::NavigationChanged(index) = message {
            self.active_component = HomeCid::from(index as usize);

            let active_component = Cid::Home(self.active_component.clone());
            app.active(&active_component)?;

            self.update_shortcuts(app, self.active_component.clone())?;
        }

        self.update_context(app, context, theme, self.active_component.clone())?;

        Ok(None)
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let shortcuts_h = 1u16;
        let layout = layout::default_page(area, shortcuts_h);

        app.view(&Cid::Home(HomeCid::Navigation), frame, layout.navigation);
        app.view(
            &Cid::Home(self.active_component.clone()),
            frame,
            layout.component,
        );

        if self.active_component != HomeCid::Dashboard {
            app.view(&Cid::Home(HomeCid::Context), frame, layout.context);
        }

        app.view(&Cid::Home(HomeCid::Shortcuts), frame, layout.shortcuts);
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
    active_component: IssueCid,
    shortcuts: HashMap<IssueCid, Widget<Shortcuts>>,
}

impl IssuePage {
    pub fn new(_context: &Context, theme: &Theme, issue: (IssueId, Issue)) -> Self {
        let shortcuts = Self::build_shortcuts(theme);
        let active_component = IssueCid::List;
        
        Self {
            issue,
            active_component,
            shortcuts,
        }
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<IssueCid, Widget<Shortcuts>> {
        [
            (
                IssueCid::List,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "esc", "back"),
                        widget::common::shortcut(theme, "↑/↓", "navigate"),
                        widget::common::shortcut(theme, "enter", "show"),
                        widget::common::shortcut(theme, "n", "new issue"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                IssueCid::Details,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "esc", "back"),
                        widget::common::shortcut(theme, "↑/↓", "scroll"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                IssueCid::NewForm,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "esc", "back"),
                        widget::common::shortcut(theme, "shift + tab / tab", "navigate"),
                        widget::common::shortcut(theme, "alt + s", "submit"),
                    ],
                ),
            ),
        ]
        .iter()
        .cloned()
        .collect()
    }

    fn update_context(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        cid: IssueCid,
    ) -> Result<()> {
        use tuirealm::State;

        let context = match cid {
            IssueCid::List => {
                let state = app.state(&Cid::Issue(IssueCid::List))?;
                let progress = match state {
                    State::Tup2((StateValue::Usize(step), StateValue::Usize(total))) => {
                        Progress::Step(step.saturating_add(1), total)
                    }
                    _ => Progress::None,
                };
                let context = widget::issue::browse_context(context, theme, progress);
                Some(context)
            }
            IssueCid::Details => {
                let state = app.state(&Cid::Issue(IssueCid::Details))?;
                let progress = match state {
                    State::One(StateValue::Usize(scroll)) => Progress::Percentage(scroll),
                    _ => Progress::None,
                };
                let context = widget::issue::description_context(context, theme, progress);
                Some(context)
            }
            _ => None,
        };

        if let Some(context) = context {
            app.remount(Cid::Issue(IssueCid::Context), context.to_boxed(), vec![])?;
        }

        Ok(())
    }

    fn update_shortcuts(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: IssueCid,
    ) -> Result<()> {
        if let Some(shortcuts) = self.shortcuts.get(&cid) {
            app.remount(
                Cid::Issue(IssueCid::Shortcuts),
                shortcuts.clone().to_boxed(),
                vec![],
            )?;
        }
        Ok(())
    }

    fn activate(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: IssueCid,
    ) -> Result<()> {
        self.active_component = cid;
        app.active(&Cid::Issue(self.active_component.clone()))?;

        Ok(())
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
        let comments = issue.comments().collect::<Vec<_>>();

        let list = widget::issue::list(context, theme, (*id, issue.clone())).to_boxed();
        let details = widget::issue::details(
            context,
            theme,
            (*id, issue.clone()),
            comments.first().copied(),
        )
        .to_boxed();

        app.remount(Cid::Issue(IssueCid::List), list, vec![])?;
        app.remount(Cid::Issue(IssueCid::Details), details, vec![])?;

        app.active(&Cid::Issue(self.active_component.clone()))?;

        self.update_shortcuts(app, self.active_component.clone())?;
        self.update_context(app, context, theme, self.active_component.clone())?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Issue(IssueCid::List))?;
        app.umount(&Cid::Issue(IssueCid::Details))?;
        app.umount(&Cid::Issue(IssueCid::Context))?;
        if app.mounted(&Cid::Issue(IssueCid::NewForm)) {
            app.umount(&Cid::Issue(IssueCid::NewForm))?;
        }
        app.umount(&Cid::Issue(IssueCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
        message: Message,
    ) -> Result<Option<Message>> {
        match message {
            Message::Issue(IssueMessage::Changed(id)) => {
                let repo = context.repository();
                if let Some(issue) = cob::issue::find(repo, &id)? {
                    let comments = issue.comments().collect::<Vec<_>>();
                    let details = widget::issue::details(
                        context,
                        theme,
                        (id, issue.clone()),
                        comments.first().copied(),
                    )
                    .to_boxed();
                    app.remount(Cid::Issue(IssueCid::Details), details, vec![])?;
                }
            }
            Message::Issue(IssueMessage::Focus(cid)) => {
                self.activate(app, cid)?;
                self.update_shortcuts(app, self.active_component.clone())?;
            }
            Message::Issue(IssueMessage::OpenPopup(cid)) => {
                if cid == IssueCid::NewForm {
                    let new_form = widget::issue::new_form(context, theme).to_boxed();
                    app.remount(Cid::Issue(cid.clone()), new_form, vec![])?;
                    app.active(&Cid::Issue(cid.clone()))?;

                    self.update_shortcuts(app, cid)?;
                }
            }
            Message::Issue(IssueMessage::ClosePopup(cid)) => {
                app.blur()?;
                app.umount(&Cid::Issue(cid))?;

                self.update_shortcuts(app, IssueCid::List)?;
            }
            _ => {}
        }

        self.update_context(app, context, theme, self.active_component.clone())?;

        Ok(None)
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let shortcuts_h = 1u16;
        let layout = layout::issue_page(area, shortcuts_h);

        app.view(&Cid::Issue(IssueCid::List), frame, layout.left);

        if app.mounted(&Cid::Issue(IssueCid::NewForm)) {
            app.view(&Cid::Issue(IssueCid::NewForm), frame, layout.right);
        } else {
            app.view(&Cid::Issue(IssueCid::Details), frame, layout.right);
        }

        app.view(&Cid::Issue(IssueCid::Context), frame, layout.context);
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
/// Patch detail page
///
pub struct PatchView {
    active_component: PatchCid,
    patch: (PatchId, Patch),
    shortcuts: HashMap<PatchCid, Widget<Shortcuts>>,
}

impl PatchView {
    pub fn new(theme: Theme, patch: (PatchId, Patch)) -> Self {
        let shortcuts = Self::build_shortcuts(&theme);
        PatchView {
            active_component: PatchCid::Activity,
            patch,
            shortcuts,
        }
    }

    fn build_shortcuts(theme: &Theme) -> HashMap<PatchCid, Widget<Shortcuts>> {
        [
            (
                PatchCid::Activity,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "esc", "back"),
                        widget::common::shortcut(theme, "tab", "section"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
            (
                PatchCid::Files,
                widget::common::shortcuts(
                    theme,
                    vec![
                        widget::common::shortcut(theme, "esc", "back"),
                        widget::common::shortcut(theme, "tab", "section"),
                        widget::common::shortcut(theme, "q", "quit"),
                    ],
                ),
            ),
        ]
        .iter()
        .cloned()
        .collect()
    }

    fn update_shortcuts(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        cid: PatchCid,
    ) -> Result<()> {
        if let Some(shortcuts) = self.shortcuts.get(&cid) {
            app.remount(
                Cid::Patch(PatchCid::Shortcuts),
                shortcuts.clone().to_boxed(),
                vec![],
            )?;
        }
        Ok(())
    }
}

impl ViewPage for PatchView {
    fn mount(
        &self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        context: &Context,
        theme: &Theme,
    ) -> Result<()> {
        let navigation = widget::patch::navigation(theme).to_boxed();
        let activity = widget::patch::activity(theme).to_boxed();
        let files = widget::patch::files(theme).to_boxed();
        let context = widget::patch::context(context, theme, self.patch.clone()).to_boxed();

        app.remount(Cid::Patch(PatchCid::Navigation), navigation, vec![])?;
        app.remount(Cid::Patch(PatchCid::Activity), activity, vec![])?;
        app.remount(Cid::Patch(PatchCid::Files), files, vec![])?;
        app.remount(Cid::Patch(PatchCid::Context), context, vec![])?;

        let active_component = Cid::Patch(self.active_component.clone());
        app.active(&active_component)?;
        self.update_shortcuts(app, self.active_component.clone())?;

        Ok(())
    }

    fn unmount(&self, app: &mut Application<Cid, Message, NoUserEvent>) -> Result<()> {
        app.umount(&Cid::Patch(PatchCid::Navigation))?;
        app.umount(&Cid::Patch(PatchCid::Activity))?;
        app.umount(&Cid::Patch(PatchCid::Files))?;
        app.umount(&Cid::Patch(PatchCid::Context))?;
        app.umount(&Cid::Patch(PatchCid::Shortcuts))?;
        Ok(())
    }

    fn update(
        &mut self,
        app: &mut Application<Cid, Message, NoUserEvent>,
        _context: &Context,
        _theme: &Theme,
        message: Message,
    ) -> Result<Option<Message>> {
        if let Message::NavigationChanged(index) = message {
            self.active_component = PatchCid::from(index as usize);

            let active_component = Cid::Patch(self.active_component.clone());
            app.active(&active_component)?;
            self.update_shortcuts(app, self.active_component.clone())?;
        }

        Ok(None)
    }

    fn view(&mut self, app: &mut Application<Cid, Message, NoUserEvent>, frame: &mut Frame) {
        let area = frame.size();
        let shortcuts_h = 1u16;
        let layout = layout::default_page(area, shortcuts_h);

        app.view(&Cid::Patch(PatchCid::Navigation), frame, layout.navigation);
        app.view(
            &Cid::Patch(self.active_component.clone()),
            frame,
            layout.component,
        );
        app.view(&Cid::Patch(PatchCid::Context), frame, layout.context);
        app.view(&Cid::Patch(PatchCid::Shortcuts), frame, layout.shortcuts);
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
