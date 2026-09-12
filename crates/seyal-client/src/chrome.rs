//! Portable shell chrome: agents, inspector, attention, and left-panel mode.
//!
//! This module derives inspector/attention projections from authoritative
//! [`crate::shell::ShellSnapshot`] plus activity rows supplied by the host.
//! It does not own Workspace/Tab/Pane identities, fabricate Runtime telemetry,
//! or implement an agent provider. Hosts dispatch [`ChromeAction`] and render
//! [`ChromeSnapshot`].

use std::collections::HashMap;
use std::fmt;

use seyal_core::{TabId, WorkspaceId};

use crate::shell::{LayoutDescription, ShellSnapshot};

/// Product chrome for the left context list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftPanelMode {
    Workspaces,
    Tabs,
}

/// Product inspector filter. Filtering never invents rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorMode {
    Context,
    Workspace,
    Tab,
    Pane,
}

/// Display state for one agent row. This is not Runtime telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentActivity {
    Running,
    Waiting,
    Attention,
    Idle,
}

impl AgentActivity {
    fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Waiting => "Waiting",
            Self::Attention => "Attention",
            Self::Idle => "Idle",
        }
    }
}

/// Host-facing agent identity. Not a Runtime/Execution/Workspace authority.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Host-facing attention identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttentionId(String);

impl AttentionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRecord {
    pub id: AgentId,
    pub name: String,
    pub activity: AgentActivity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttentionItem {
    pub id: AttentionId,
    pub title: String,
    pub detail: String,
    pub workspace: Option<WorkspaceId>,
    pub tab: Option<TabId>,
    pub agent: Option<AgentId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectorRow {
    pub id: String,
    pub section: String,
    pub label: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeError {
    UnknownAgent,
    UnknownAttention,
    UnknownWorkspace,
    UnknownTab,
}

impl ChromeError {
    fn message(self) -> &'static str {
        match self {
            Self::UnknownAgent => "Unknown agent.",
            Self::UnknownAttention => "Unknown attention item.",
            Self::UnknownWorkspace => "Attention target Workspace does not exist.",
            Self::UnknownTab => "Attention target Tab does not exist.",
        }
    }
}

impl fmt::Display for ChromeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChromeAction {
    SetLeftPanel(LeftPanelMode),
    SetInspectorMode(InspectorMode),
    SelectAgent {
        id: AgentId,
    },
    OpenAttention {
        id: AttentionId,
    },
    ReplaceAgents {
        workspace: WorkspaceId,
        agents: Vec<AgentRecord>,
    },
    ReplaceAttention {
        items: Vec<AttentionItem>,
    },
    /// Host applied a Workspace/Tab/Pane navigation action. Clears agent
    /// selection without inventing new composition identities.
    ContextNavigated,
}

/// Navigation the host must apply to [`crate::shell::ShellState`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChromeEffect {
    pub select_workspace: Option<WorkspaceId>,
    pub select_tab: Option<TabId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChromeSnapshot {
    pub left_panel: LeftPanelMode,
    pub inspector_mode: InspectorMode,
    pub selected_agent: Option<AgentId>,
    pub agents: Vec<AgentRecord>,
    pub inspector_rows: Vec<InspectorRow>,
    pub visible_inspector_rows: Vec<InspectorRow>,
    pub attention_items: Vec<AttentionItem>,
    pub last_error: Option<ChromeError>,
}

/// Authoritative chrome/inspector/attention product state.
#[derive(Clone, Debug)]
pub struct ChromeState {
    left_panel: LeftPanelMode,
    inspector_mode: InspectorMode,
    selected_agent: Option<AgentId>,
    agents: HashMap<WorkspaceId, Vec<AgentRecord>>,
    attention: Vec<AttentionItem>,
    last_error: Option<ChromeError>,
}

impl Default for ChromeState {
    fn default() -> Self {
        Self {
            left_panel: LeftPanelMode::Workspaces,
            inspector_mode: InspectorMode::Context,
            selected_agent: None,
            agents: HashMap::new(),
            attention: Vec::new(),
            last_error: None,
        }
    }
}

impl ChromeState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        action: ChromeAction,
        shell: &ShellSnapshot,
    ) -> Result<ChromeEffect, ChromeError> {
        self.last_error = None;
        match action {
            ChromeAction::SetLeftPanel(mode) => {
                self.left_panel = mode;
                self.selected_agent = None;
                Ok(ChromeEffect::default())
            }
            ChromeAction::SetInspectorMode(mode) => {
                self.inspector_mode = mode;
                Ok(ChromeEffect::default())
            }
            ChromeAction::SelectAgent { id } => {
                if !self
                    .agents_for(shell.active_workspace)
                    .iter()
                    .any(|agent| agent.id == id)
                {
                    return self.fail(ChromeError::UnknownAgent);
                }
                self.selected_agent = Some(id);
                Ok(ChromeEffect::default())
            }
            ChromeAction::OpenAttention { id } => self.open_attention(&id, shell),
            ChromeAction::ReplaceAgents { workspace, agents } => {
                self.agents.insert(workspace, agents);
                if let Some(selected) = &self.selected_agent
                    && !self
                        .agents_for(shell.active_workspace)
                        .iter()
                        .any(|agent| agent.id == *selected)
                {
                    self.selected_agent = None;
                }
                Ok(ChromeEffect::default())
            }
            ChromeAction::ReplaceAttention { items } => {
                self.attention = items;
                Ok(ChromeEffect::default())
            }
            ChromeAction::ContextNavigated => {
                self.selected_agent = None;
                Ok(ChromeEffect::default())
            }
        }
    }

    pub fn snapshot(&self, shell: &ShellSnapshot) -> ChromeSnapshot {
        let inspector_rows = self.inspector_rows(shell);
        let visible_inspector_rows = filter_rows(&inspector_rows, self.inspector_mode);
        ChromeSnapshot {
            left_panel: self.left_panel,
            inspector_mode: self.inspector_mode,
            selected_agent: self.selected_agent.clone(),
            agents: self.agents_for(shell.active_workspace).to_vec(),
            inspector_rows,
            visible_inspector_rows,
            attention_items: self.attention.clone(),
            last_error: self.last_error,
        }
    }

    fn open_attention(
        &mut self,
        id: &AttentionId,
        shell: &ShellSnapshot,
    ) -> Result<ChromeEffect, ChromeError> {
        let index = self
            .attention
            .iter()
            .position(|item| item.id == *id)
            .ok_or_else(|| {
                self.last_error = Some(ChromeError::UnknownAttention);
                ChromeError::UnknownAttention
            })?;
        let item = self.attention[index].clone();
        if let Some(workspace) = item.workspace
            && !shell.workspaces.iter().any(|row| row.id == workspace)
        {
            return self.fail(ChromeError::UnknownWorkspace);
        }
        if let Some(tab) = item.tab {
            let tab_known = match item.workspace {
                Some(workspace) if workspace != shell.active_workspace => true,
                _ => shell.tabs.iter().any(|row| row.id == tab),
            };
            if !tab_known && item.workspace.is_none() {
                return self.fail(ChromeError::UnknownTab);
            }
        }
        if let Some(agent) = &item.agent {
            let workspace = item.workspace.unwrap_or(shell.active_workspace);
            if !self
                .agents_for(workspace)
                .iter()
                .any(|row| row.id == *agent)
            {
                return self.fail(ChromeError::UnknownAgent);
            }
            self.selected_agent = Some(agent.clone());
        } else {
            self.selected_agent = None;
        }
        self.attention.remove(index);
        Ok(ChromeEffect {
            select_workspace: item.workspace,
            select_tab: item.tab,
        })
    }

    fn inspector_rows(&self, shell: &ShellSnapshot) -> Vec<InspectorRow> {
        let workspace = shell
            .workspaces
            .iter()
            .find(|row| row.id == shell.active_workspace);
        let tab = shell.tabs.iter().find(|row| row.id == shell.active_tab);
        let pane = shell.panes.iter().find(|row| row.id == shell.focused_pane);
        if let Some(selected) = &self.selected_agent
            && let Some(agent) = self
                .agents_for(shell.active_workspace)
                .iter()
                .find(|row| row.id == *selected)
        {
            return vec![
                row("agent-name", "Agent", "Name", agent.name.clone()),
                row(
                    "agent-state",
                    "Agent",
                    "State",
                    agent.activity.label().to_owned(),
                ),
                row(
                    "agent-workspace",
                    "Workspace",
                    "Name",
                    workspace
                        .map(|item| item.name.clone())
                        .unwrap_or_else(|| "—".to_owned()),
                ),
            ];
        }
        vec![
            row(
                "workspace-name",
                "Workspace",
                "Name",
                workspace
                    .map(|item| item.name.clone())
                    .unwrap_or_else(|| "—".to_owned()),
            ),
            row(
                "workspace-path",
                "Workspace",
                "Path",
                workspace
                    .and_then(|item| item.detail.clone())
                    .unwrap_or_else(|| "—".to_owned()),
            ),
            row(
                "tab-name",
                "Tab",
                "Name",
                tab.map(|item| item.title.clone())
                    .unwrap_or_else(|| "—".to_owned()),
            ),
            row(
                "tab-panes",
                "Tab",
                "Panes",
                tab.map(|item| item.pane_count.to_string())
                    .unwrap_or_else(|| "—".to_owned()),
            ),
            row(
                "tab-layout",
                "Tab",
                "Layout",
                layout_label(shell.layout).to_owned(),
            ),
            row(
                "pane-name",
                "Active Pane",
                "Pane",
                pane.map(|item| item.title.clone())
                    .unwrap_or_else(|| "—".to_owned()),
            ),
            row("pane-focus", "Active Pane", "Focus", "Focused".to_owned()),
        ]
    }

    fn agents_for(&self, workspace: WorkspaceId) -> &[AgentRecord] {
        self.agents.get(&workspace).map_or(&[], Vec::as_slice)
    }

    fn fail<T>(&mut self, error: ChromeError) -> Result<T, ChromeError> {
        self.last_error = Some(error);
        Err(error)
    }
}

fn row(id: &str, section: &str, label: &str, value: String) -> InspectorRow {
    InspectorRow {
        id: id.to_owned(),
        section: section.to_owned(),
        label: label.to_owned(),
        value,
    }
}

fn filter_rows(rows: &[InspectorRow], mode: InspectorMode) -> Vec<InspectorRow> {
    match mode {
        InspectorMode::Context => rows.to_vec(),
        InspectorMode::Workspace => rows
            .iter()
            .filter(|row| row.section == "Workspace")
            .cloned()
            .collect(),
        InspectorMode::Tab => rows
            .iter()
            .filter(|row| row.section == "Tab")
            .cloned()
            .collect(),
        InspectorMode::Pane => rows
            .iter()
            .filter(|row| row.section == "Active Pane")
            .cloned()
            .collect(),
    }
}

fn layout_label(layout: LayoutDescription) -> &'static str {
    match layout {
        LayoutDescription::Single => "Single pane",
        LayoutDescription::SplitRight => "Split right",
        LayoutDescription::SplitDown => "Split down",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{ShellAction, ShellPaneSeed, ShellState, ShellTabSeed, ShellWorkspaceSeed};
    use seyal_core::{PaneId, TabId, WorkspaceId};

    fn workspace(tag: u8) -> WorkspaceId {
        WorkspaceId::from_bytes([tag; 16])
    }

    fn tab(tag: u8) -> TabId {
        TabId::from_bytes([tag; 16])
    }

    fn pane(tag: u8) -> PaneId {
        PaneId::from_bytes([tag; 16])
    }

    fn seed_shell() -> ShellState {
        let local = workspace(1);
        let other = workspace(2);
        let local_tab = tab(1);
        let agent_tab = tab(2);
        let other_tab = tab(3);
        ShellState::from_workspaces(
            vec![
                ShellWorkspaceSeed {
                    id: local,
                    name: "Seyal OSS".into(),
                    detail: Some("~/Projects/seyal".into()),
                    attention: false,
                    active_tab: local_tab,
                    tabs: vec![
                        ShellTabSeed {
                            id: local_tab,
                            title: "Core Terminal".into(),
                            attention: false,
                            pane: ShellPaneSeed {
                                id: pane(1),
                                title: "Pane 1".into(),
                                allows_implicit_execution_bootstrap: true,
                            },
                        },
                        ShellTabSeed {
                            id: agent_tab,
                            title: "Agent Development".into(),
                            attention: false,
                            pane: ShellPaneSeed {
                                id: pane(2),
                                title: "Pane 2".into(),
                                allows_implicit_execution_bootstrap: false,
                            },
                        },
                    ],
                },
                ShellWorkspaceSeed {
                    id: other,
                    name: "Payments".into(),
                    detail: Some("~/Projects/payments".into()),
                    attention: true,
                    active_tab: other_tab,
                    tabs: vec![ShellTabSeed {
                        id: other_tab,
                        title: "API".into(),
                        attention: false,
                        pane: ShellPaneSeed {
                            id: pane(3),
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
        .expect("seed")
    }

    fn claude() -> AgentRecord {
        AgentRecord {
            id: AgentId::new("agent-claude"),
            name: "Claude Code".into(),
            activity: AgentActivity::Running,
        }
    }

    fn seed_agents(chrome: &mut ChromeState, shell: &ShellSnapshot) {
        chrome
            .apply(
                ChromeAction::ReplaceAgents {
                    workspace: shell.active_workspace,
                    agents: vec![
                        claude(),
                        AgentRecord {
                            id: AgentId::new("agent-codex"),
                            name: "Codex".into(),
                            activity: AgentActivity::Attention,
                        },
                    ],
                },
                shell,
            )
            .unwrap();
    }

    #[test]
    fn inspector_does_not_fabricate_runtime_telemetry() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        seed_agents(&mut chrome, &snap);
        let rows = chrome.snapshot(&snap).inspector_rows;
        let labels: Vec<_> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Name", "Path", "Name", "Panes", "Layout", "Pane", "Focus"]
        );
        assert!(rows.iter().all(|row| {
            !row.label.contains("CPU")
                && !row.label.contains("RSS")
                && !row.label.contains("PTY")
                && !row.value.contains("pid")
        }));
        assert_eq!(rows[0].value, "Seyal OSS");
        assert_eq!(rows[1].value, "~/Projects/seyal");
    }

    #[test]
    fn inspector_mode_filters_existing_context_only() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        chrome
            .apply(ChromeAction::SetInspectorMode(InspectorMode::Tab), &snap)
            .unwrap();
        let visible = chrome.snapshot(&snap).visible_inspector_rows;
        assert!(visible.iter().all(|row| row.section == "Tab"));
        assert_eq!(visible.len(), 3);
        assert!(!visible.iter().any(|row| row.section == "Workspace"));
    }

    #[test]
    fn selected_agent_uses_activity_rows_not_runtime_metrics() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        seed_agents(&mut chrome, &snap);
        chrome
            .apply(
                ChromeAction::SelectAgent {
                    id: AgentId::new("agent-claude"),
                },
                &snap,
            )
            .unwrap();
        let rows = chrome.snapshot(&snap).inspector_rows;
        assert_eq!(rows[0].section, "Agent");
        assert_eq!(rows[0].value, "Claude Code");
        assert_eq!(rows[1].value, "Running");
        assert!(rows.iter().all(|row| row.label != "CPU"));
    }

    #[test]
    fn left_panel_mode_does_not_invent_workspace_or_tab_identities() {
        let mut shell = seed_shell();
        let before = shell.snapshot();
        let mut chrome = ChromeState::new();
        chrome
            .apply(ChromeAction::SetLeftPanel(LeftPanelMode::Tabs), &before)
            .unwrap();
        let after_shell = shell.snapshot();
        assert_eq!(after_shell.active_workspace, before.active_workspace);
        assert_eq!(after_shell.active_tab, before.active_tab);
        assert_eq!(after_shell.workspaces, before.workspaces);
        assert_eq!(
            chrome.snapshot(&after_shell).left_panel,
            LeftPanelMode::Tabs
        );
        assert!(chrome.snapshot(&after_shell).selected_agent.is_none());
        shell.apply(ShellAction::SelectTab { id: tab(2) }).unwrap();
        chrome
            .apply(ChromeAction::ContextNavigated, &shell.snapshot())
            .unwrap();
        assert_eq!(shell.snapshot().active_tab, tab(2));
        assert!(chrome.snapshot(&shell.snapshot()).selected_agent.is_none());
    }

    #[test]
    fn attention_item_navigates_then_dismisses() {
        let mut shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        seed_agents(&mut chrome, &snap);
        chrome
            .apply(
                ChromeAction::ReplaceAttention {
                    items: vec![AttentionItem {
                        id: AttentionId::new("attention-preview-tab"),
                        title: "Preview attention item".into(),
                        detail: "Open Agent Development".into(),
                        workspace: Some(workspace(1)),
                        tab: Some(tab(2)),
                        agent: Some(AgentId::new("agent-codex")),
                    }],
                },
                &snap,
            )
            .unwrap();
        let effect = chrome
            .apply(
                ChromeAction::OpenAttention {
                    id: AttentionId::new("attention-preview-tab"),
                },
                &snap,
            )
            .unwrap();
        assert_eq!(effect.select_workspace, Some(workspace(1)));
        assert_eq!(effect.select_tab, Some(tab(2)));
        if let Some(id) = effect.select_workspace {
            shell.apply(ShellAction::SelectWorkspace { id }).unwrap();
        }
        if let Some(id) = effect.select_tab {
            shell.apply(ShellAction::SelectTab { id }).unwrap();
        }
        let after = chrome.snapshot(&shell.snapshot());
        assert!(after.attention_items.is_empty());
        assert_eq!(after.selected_agent, Some(AgentId::new("agent-codex")));
        assert_eq!(shell.snapshot().active_tab, tab(2));
    }

    #[test]
    fn unknown_attention_or_agent_fails_closed() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        seed_agents(&mut chrome, &snap);
        assert_eq!(
            chrome.apply(
                ChromeAction::OpenAttention {
                    id: AttentionId::new("missing")
                },
                &snap
            ),
            Err(ChromeError::UnknownAttention)
        );
        assert_eq!(
            chrome.apply(
                ChromeAction::SelectAgent {
                    id: AgentId::new("ghost")
                },
                &snap
            ),
            Err(ChromeError::UnknownAgent)
        );
        assert!(chrome.snapshot(&snap).attention_items.is_empty());
        assert!(chrome.snapshot(&snap).selected_agent.is_none());
    }

    #[test]
    fn attention_unknown_workspace_does_not_dismiss() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        chrome
            .apply(
                ChromeAction::ReplaceAttention {
                    items: vec![AttentionItem {
                        id: AttentionId::new("bad-workspace"),
                        title: "Ghost".into(),
                        detail: "Missing".into(),
                        workspace: Some(workspace(9)),
                        tab: None,
                        agent: None,
                    }],
                },
                &snap,
            )
            .unwrap();
        assert_eq!(
            chrome.apply(
                ChromeAction::OpenAttention {
                    id: AttentionId::new("bad-workspace")
                },
                &snap
            ),
            Err(ChromeError::UnknownWorkspace)
        );
        assert_eq!(chrome.snapshot(&snap).attention_items.len(), 1);
    }
}
