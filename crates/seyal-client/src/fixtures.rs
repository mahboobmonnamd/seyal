//! Shared product fixtures for Rust and future native host tests.
//!
//! These builders are the replacement for Swift `makePreview` / `makeProduction`.
//! They must not be used from the PTY→VT→damage path.

use crate::product::ProductSession;
use crate::shell::{ShellPaneSeed, ShellState, ShellTabSeed, ShellWorkspaceSeed};
use seyal_core::{PaneId, TabId, WorkspaceId};

/// M001 production fixture: one workspace, one tab, one pane.
pub fn production_session() -> ProductSession {
    ProductSession::m001_local("local")
}

/// Deterministic two-workspace fixture used by chrome/inspector tests.
pub fn preview_shell() -> ShellState {
    let local = WorkspaceId::from_bytes([0xA1; 16]);
    let other = WorkspaceId::from_bytes([0xA2; 16]);
    let local_tab = TabId::from_bytes([0xB1; 16]);
    let other_tab = TabId::from_bytes([0xB2; 16]);
    ShellState::from_workspaces(
        vec![
            ShellWorkspaceSeed {
                id: local,
                name: "Seyal OSS".into(),
                detail: Some("~/Projects/seyal".into()),
                attention: false,
                active_tab: local_tab,
                tabs: vec![ShellTabSeed {
                    id: local_tab,
                    title: "Core Terminal".into(),
                    attention: false,
                    pane: ShellPaneSeed {
                        id: PaneId::from_bytes([0xC1; 16]),
                        title: "Pane 1".into(),
                        allows_implicit_execution_bootstrap: true,
                    },
                }],
            },
            ShellWorkspaceSeed {
                id: other,
                name: "Payments".into(),
                detail: Some("~/Projects/payments".into()),
                attention: false,
                active_tab: other_tab,
                tabs: vec![ShellTabSeed {
                    id: other_tab,
                    title: "API".into(),
                    attention: false,
                    pane: ShellPaneSeed {
                        id: PaneId::from_bytes([0xC2; 16]),
                        title: "Pane 1".into(),
                        allows_implicit_execution_bootstrap: false,
                    },
                }],
            },
        ],
        local,
        true,
        true,
    )
    .expect("preview fixture")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_fixtures_replace_swift_preview_construction() {
        let production = production_session().snapshot();
        assert_eq!(production.workspace_name, "Local");
        assert!(production.draft.is_empty());
        let preview = preview_shell().snapshot();
        assert_eq!(preview.workspaces.len(), 2);
        assert_eq!(preview.workspaces[0].name, "Seyal OSS");
    }
}
