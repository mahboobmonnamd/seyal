//! Combined headed product session for the thin native host.
//!
//! Owns the portable reducers and exposes one action/snapshot seam. It is not
//! a second Runtime, PTY, VT, or BlockTimeline.

use crate::chrome::{ChromeAction, ChromeState};
use crate::composer::{ComposerAction, ComposerState};
use crate::presentation::{PresentationMode, PresentationSession};
use crate::shell::{ShellSnapshot, ShellState};
use seyal_core::PaneId;

/// One headed product session. Hosts dispatch [`ProductAction`] and render
/// [`ProductSnapshot`].
#[derive(Clone, Debug)]
pub struct ProductSession {
    shell: ShellState,
    chrome: ChromeState,
    composer: ComposerState,
    presentation: PresentationSession,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductAction {
    SetDraft(String),
    Submit,
    SetLeftPanelWorkspaces,
    SetLeftPanelTabs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSnapshot {
    pub shell: ShellSnapshot,
    pub workspace_name: String,
    pub tab_title: String,
    pub pane_title: String,
    pub draft: String,
    pub can_submit: bool,
    pub composer_hidden: bool,
    pub composer_busy: bool,
    pub composer_epoch: u64,
    pub presentation: PresentationMode,
    pub left_panel_tabs: bool,
}

impl ProductSession {
    pub fn m001_local(detail: impl Into<String>) -> Self {
        let shell = ShellState::m001_local(detail);
        let focused = shell.snapshot().focused_pane;
        let mut composer = ComposerState::new();
        composer
            .apply(ComposerAction::EnsurePane { pane: focused })
            .expect("focused pane");
        Self {
            chrome: ChromeState::new(),
            composer,
            presentation: PresentationSession::new(None, PresentationMode::Flow),
            shell,
        }
    }

    pub fn apply(&mut self, action: ProductAction) -> Result<(), String> {
        let pane = self.focused_pane();
        match action {
            ProductAction::SetDraft(text) => {
                let epoch = self
                    .composer
                    .snapshot(pane)
                    .map_err(|e| e.to_string())?
                    .epoch;
                self.composer
                    .apply(ComposerAction::SetDraft { pane, text, epoch })
                    .map_err(|e| e.to_string())?;
            }
            ProductAction::Submit => {
                let epoch = self
                    .composer
                    .snapshot(pane)
                    .map_err(|e| e.to_string())?
                    .epoch;
                self.composer
                    .apply(ComposerAction::Submit { pane, epoch })
                    .map_err(|e| e.to_string())?;
            }
            ProductAction::SetLeftPanelWorkspaces => {
                self.chrome
                    .apply(
                        ChromeAction::SetLeftPanel(crate::chrome::LeftPanelMode::Workspaces),
                        &self.shell.snapshot(),
                    )
                    .map_err(|e| e.to_string())?;
            }
            ProductAction::SetLeftPanelTabs => {
                self.chrome
                    .apply(
                        ChromeAction::SetLeftPanel(crate::chrome::LeftPanelMode::Tabs),
                        &self.shell.snapshot(),
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn snapshot(&self) -> ProductSnapshot {
        let shell = self.shell.snapshot();
        let chrome = self.chrome.snapshot(&shell);
        let composer = self
            .composer
            .snapshot(shell.focused_pane)
            .expect("focused composer");
        let workspace_name = shell
            .workspaces
            .iter()
            .find(|row| row.id == shell.active_workspace)
            .map(|row| row.name.clone())
            .unwrap_or_else(|| "—".to_owned());
        let tab_title = shell
            .tabs
            .iter()
            .find(|row| row.id == shell.active_tab)
            .map(|row| row.title.clone())
            .unwrap_or_else(|| "—".to_owned());
        let pane_title = shell
            .panes
            .iter()
            .find(|row| row.id == shell.focused_pane)
            .map(|row| row.title.clone())
            .unwrap_or_else(|| "—".to_owned());
        ProductSnapshot {
            shell,
            workspace_name,
            tab_title,
            pane_title,
            draft: composer.draft,
            can_submit: composer.can_submit,
            composer_hidden: matches!(composer.mode, crate::composer::ComposerMode::Hidden),
            composer_busy: matches!(composer.mode, crate::composer::ComposerMode::Busy { .. }),
            composer_epoch: composer.epoch,
            presentation: self.presentation.snapshot().mode,
            left_panel_tabs: chrome.left_panel == crate::chrome::LeftPanelMode::Tabs,
        }
    }

    fn focused_pane(&self) -> PaneId {
        self.shell.snapshot().focused_pane
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m001_session_exposes_flow_composer_without_swift_state() {
        let session = ProductSession::m001_local("~/Projects/seyal");
        let snap = session.snapshot();
        assert_eq!(snap.workspace_name, "Local");
        assert_eq!(snap.tab_title, "Terminal");
        assert_eq!(snap.presentation, PresentationMode::Flow);
        assert!(!snap.composer_hidden);
        assert!(!snap.can_submit);
        assert!(snap.draft.is_empty());
    }

    #[test]
    fn draft_and_submit_are_rust_actions() {
        let mut session = ProductSession::m001_local("local");
        session
            .apply(ProductAction::SetDraft("echo hi".into()))
            .unwrap();
        assert_eq!(session.snapshot().draft, "echo hi");
        assert!(session.snapshot().can_submit);
        session.apply(ProductAction::Submit).unwrap();
        assert_eq!(session.snapshot().draft, "echo hi");
        assert!(session.snapshot().composer_busy);
        assert!(!session.snapshot().can_submit);
    }

    #[test]
    fn left_panel_action_does_not_invent_workspace_identities() {
        let mut session = ProductSession::m001_local("local");
        let before = session.snapshot().shell.active_workspace;
        session.apply(ProductAction::SetLeftPanelTabs).unwrap();
        let after = session.snapshot();
        assert!(after.left_panel_tabs);
        assert_eq!(after.shell.active_workspace, before);
        assert_eq!(after.shell.workspaces.len(), 1);
    }
}
