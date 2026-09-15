//! Portable shell chrome: agents, inspector, attention, left-panel mode,
//! center surface mode, command palette, and which shell regions are visible.
//! M001 first UI recedes sidebar/inspector/tab strip
//! (`docs/architecture/ui/M001-FIRST-UI-DESIGN.md`).
//!
//! This module derives inspector/attention projections from authoritative
//! [`crate::shell::ShellSnapshot`] plus activity rows supplied by the host.
//! It does not own Workspace/Tab/Pane identities, fabricate Runtime telemetry,
//! or implement an agent provider. Hosts dispatch [`ChromeAction`] and render
//! [`ChromeSnapshot`]. The global command palette catalog lives in
//! [`crate::chrome_palette`]; this module owns open/query/selection state.

use std::collections::HashMap;
use std::fmt;

use seyal_core::{BlockId, TabId, WorkspaceId};

use crate::chrome_palette::{
    clamp_selected, filter_commands, move_selected, PaletteCommandId, PaletteCommandRow,
};
use crate::composer::{BlockPresentationState, BlockProjection};
use crate::shell::{LayoutDescription, ShellSnapshot};

/// Product chrome for the left context list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftPanelMode {
    Workspaces,
    Tabs,
}

/// Product chrome for the center surface (Core Terminal default vs Agents).
///
/// Agents mode projects the same agent authority as the left inventory; it is
/// not a provider and does not invent sessions. Opening Agents does not affect
/// PTY/VT/execution progress (`docs/architecture/ui/M001-AGENTS-VIEW.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CenterSurface {
    /// Default Core Terminal center (Blocks / multipane / live surface).
    Core,
    /// Agents management list for the active Workspace agent rows.
    Agents,
}

/// Product inspector filter. Filtering never invents rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorMode {
    Context,
    Workspace,
    Tab,
    Pane,
    /// Focused-pane Block projection rows only (empty when none exist).
    Blocks,
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
    /// Palette run with no selectable filtered command (fail-closed).
    EmptyPaletteSelection,
    /// Inspector Block selection that is not on the focused Pane projection.
    UnknownBlock,
}

impl ChromeError {
    fn message(self) -> &'static str {
        match self {
            Self::UnknownAgent => "Unknown agent.",
            Self::UnknownAttention => "Unknown attention item.",
            Self::UnknownWorkspace => "Attention target Workspace does not exist.",
            Self::UnknownTab => "Attention target Tab does not exist.",
            Self::EmptyPaletteSelection => "No palette command is selected.",
            Self::UnknownBlock => "Unknown Block.",
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
    /// Switch the center surface between Core Terminal and Agents.
    SetCenterSurface(CenterSurface),
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
    /// Shell-region visibility. Core Terminal chrome is visible by default;
    /// hosts may still recede regions for a denser surface.
    SetShellVisibility {
        left: bool,
        inspector: bool,
        tab_strip: bool,
    },
    /// Attention popover open/closed. Presentation chrome owned in Rust so
    /// keyboard/menu toggles share one authoritative bit with the thin host.
    SetAttentionPopover {
        open: bool,
    },
    /// Global command palette open/closed. Opening resets query/selection.
    SetPaletteOpen {
        open: bool,
    },
    SetPaletteQuery {
        query: String,
    },
    PaletteMove {
        delta: i32,
    },
    /// Run the selected filtered command. Fails closed when none is selectable.
    PaletteRun,
    /// Bind inspector to one focused-pane Block. Switches mode to Blocks.
    SelectBlock {
        id: BlockId,
    },
    /// Clear Block selection and return inspector to Context.
    ClearBlockSelection,
}

/// Navigation / palette dispatch the host (App root) must apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChromeEffect {
    pub select_workspace: Option<WorkspaceId>,
    pub select_tab: Option<TabId>,
    pub palette_command: Option<PaletteCommandId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChromeSnapshot {
    pub left_panel: LeftPanelMode,
    pub inspector_mode: InspectorMode,
    pub center_surface: CenterSurface,
    pub left_visible: bool,
    pub inspector_visible: bool,
    pub tab_strip_visible: bool,
    pub attention_popover_open: bool,
    pub palette_open: bool,
    pub palette_query: String,
    pub palette_selected: usize,
    pub palette_commands: Vec<PaletteCommandRow>,
    pub selected_agent: Option<AgentId>,
    pub selected_block: Option<BlockId>,
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
    center_surface: CenterSurface,
    left_visible: bool,
    inspector_visible: bool,
    tab_strip_visible: bool,
    attention_popover_open: bool,
    palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    selected_agent: Option<AgentId>,
    selected_block: Option<BlockId>,
    agents: HashMap<WorkspaceId, Vec<AgentRecord>>,
    attention: Vec<AttentionItem>,
    last_error: Option<ChromeError>,
}

impl Default for ChromeState {
    fn default() -> Self {
        Self {
            left_panel: LeftPanelMode::Workspaces,
            inspector_mode: InspectorMode::Context,
            center_surface: CenterSurface::Core,
            // Core Terminal mockup vertical slice: workspace chrome is visible
            // by default. Hosts may still recede regions via SetShellVisibility.
            left_visible: true,
            inspector_visible: true,
            tab_strip_visible: true,
            attention_popover_open: false,
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            selected_agent: None,
            selected_block: None,
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
        self.apply_with_blocks(action, shell, &[])
    }

    /// Apply a chrome action. `focused_blocks` validates Block selection against
    /// the authoritative focused-Pane composer projection.
    pub fn apply_with_blocks(
        &mut self,
        action: ChromeAction,
        shell: &ShellSnapshot,
        focused_blocks: &[BlockProjection],
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
                if mode != InspectorMode::Blocks {
                    self.selected_block = None;
                }
                Ok(ChromeEffect::default())
            }
            ChromeAction::SetCenterSurface(mode) => {
                self.center_surface = mode;
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
                self.selected_block = None;
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
                self.selected_block = None;
                Ok(ChromeEffect::default())
            }
            ChromeAction::SetShellVisibility {
                left,
                inspector,
                tab_strip,
            } => {
                self.left_visible = left;
                self.inspector_visible = inspector;
                self.tab_strip_visible = tab_strip;
                Ok(ChromeEffect::default())
            }
            ChromeAction::SetAttentionPopover { open } => {
                self.attention_popover_open = open;
                Ok(ChromeEffect::default())
            }
            ChromeAction::SetPaletteOpen { open } => {
                self.palette_open = open;
                if open {
                    self.palette_query.clear();
                    self.palette_selected = 0;
                } else {
                    self.dismiss_palette();
                }
                Ok(ChromeEffect::default())
            }
            ChromeAction::SetPaletteQuery { query } => {
                self.palette_query = query;
                let len = filter_commands(&self.palette_query).len();
                self.palette_selected = clamp_selected(self.palette_selected, len);
                Ok(ChromeEffect::default())
            }
            ChromeAction::PaletteMove { delta } => {
                let len = filter_commands(&self.palette_query).len();
                self.palette_selected = move_selected(self.palette_selected, delta, len);
                Ok(ChromeEffect::default())
            }
            ChromeAction::PaletteRun => self.run_palette(),
            ChromeAction::SelectBlock { id } => {
                if !focused_blocks.iter().any(|block| block.id == id) {
                    return self.fail(ChromeError::UnknownBlock);
                }
                self.selected_block = Some(id);
                self.selected_agent = None;
                self.inspector_mode = InspectorMode::Blocks;
                Ok(ChromeEffect::default())
            }
            ChromeAction::ClearBlockSelection => {
                self.selected_block = None;
                self.inspector_mode = InspectorMode::Context;
                Ok(ChromeEffect::default())
            }
        }
    }

    pub fn snapshot(&self, shell: &ShellSnapshot) -> ChromeSnapshot {
        self.snapshot_with_blocks(shell, &[])
    }

    pub fn snapshot_with_blocks(
        &self,
        shell: &ShellSnapshot,
        focused_blocks: &[BlockProjection],
    ) -> ChromeSnapshot {
        let selected_block = self
            .selected_block
            .filter(|id| focused_blocks.iter().any(|block| block.id == *id));
        let inspector_rows = self.inspector_rows(shell, focused_blocks, selected_block);
        let visible_inspector_rows = filter_rows(&inspector_rows, self.inspector_mode);
        let palette_commands = if self.palette_open {
            filter_commands(&self.palette_query)
        } else {
            Vec::new()
        };
        let palette_selected = clamp_selected(self.palette_selected, palette_commands.len());
        ChromeSnapshot {
            left_panel: self.left_panel,
            inspector_mode: self.inspector_mode,
            center_surface: self.center_surface,
            left_visible: self.left_visible,
            inspector_visible: self.inspector_visible,
            tab_strip_visible: self.tab_strip_visible,
            attention_popover_open: self.attention_popover_open,
            palette_open: self.palette_open,
            palette_query: self.palette_query.clone(),
            palette_selected,
            palette_commands,
            selected_agent: self.selected_agent.clone(),
            selected_block,
            agents: self.agents_for(shell.active_workspace).to_vec(),
            inspector_rows,
            visible_inspector_rows,
            attention_items: self.attention.clone(),
            last_error: self.last_error,
        }
    }

    fn run_palette(&mut self) -> Result<ChromeEffect, ChromeError> {
        if !self.palette_open {
            return self.fail(ChromeError::EmptyPaletteSelection);
        }
        let commands = filter_commands(&self.palette_query);
        let Some(row) = commands.get(self.palette_selected) else {
            return self.fail(ChromeError::EmptyPaletteSelection);
        };
        let Some(command) = PaletteCommandId::from_str_id(&row.id) else {
            return self.fail(ChromeError::EmptyPaletteSelection);
        };
        self.dismiss_palette();
        Ok(ChromeEffect {
            palette_command: Some(command),
            ..ChromeEffect::default()
        })
    }

    fn dismiss_palette(&mut self) {
        self.palette_open = false;
        self.palette_query.clear();
        self.palette_selected = 0;
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
        self.attention_popover_open = false;
        Ok(ChromeEffect {
            select_workspace: item.workspace,
            select_tab: item.tab,
            palette_command: None,
        })
    }

    fn inspector_rows(
        &self,
        shell: &ShellSnapshot,
        focused_blocks: &[BlockProjection],
        selected_block: Option<BlockId>,
    ) -> Vec<InspectorRow> {
        let workspace = shell
            .workspaces
            .iter()
            .find(|row| row.id == shell.active_workspace);
        let tab = shell.tabs.iter().find(|row| row.id == shell.active_tab);
        let pane = shell.panes.iter().find(|row| row.id == shell.focused_pane);
        if let Some(block_id) = selected_block
            && let Some(block) = focused_blocks.iter().find(|row| row.id == block_id)
        {
            return block_detail_rows(block, pane.map(|item| item.title.as_str()));
        }
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
        let mut rows = vec![
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
        ];
        rows.extend(block_summary_rows(focused_blocks));
        rows
    }

    fn agents_for(&self, workspace: WorkspaceId) -> &[AgentRecord] {
        self.agents.get(&workspace).map_or(&[], Vec::as_slice)
    }

    fn fail<T>(&mut self, error: ChromeError) -> Result<T, ChromeError> {
        self.last_error = Some(error);
        Err(error)
    }

    /// Drop Block selection when the focused Pane projection no longer contains it.
    pub fn retain_selected_block(&mut self, focused_blocks: &[BlockProjection]) {
        if let Some(id) = self.selected_block
            && !focused_blocks.iter().any(|block| block.id == id)
        {
            self.selected_block = None;
        }
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
        InspectorMode::Blocks => rows
            .iter()
            .filter(|row| row.section == "Block")
            .cloned()
            .collect(),
    }
}

fn block_state_label(state: BlockPresentationState) -> &'static str {
    match state {
        BlockPresentationState::Running => "Running",
        BlockPresentationState::Completed => "Completed",
        BlockPresentationState::Failed => "Failed",
    }
}

fn block_summary_rows(blocks: &[BlockProjection]) -> Vec<InspectorRow> {
    blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            row(
                &format!("block-{}-command", index),
                "Block",
                "Command",
                format!("{} · {}", block.command, block_state_label(block.state)),
            )
        })
        .collect()
}

fn block_detail_rows(block: &BlockProjection, pane_title: Option<&str>) -> Vec<InspectorRow> {
    let mut rows = vec![
        row("block-command", "Block", "Command", block.command.clone()),
        row(
            "block-state",
            "Block",
            "State",
            block_state_label(block.state).to_owned(),
        ),
        row(
            "block-start-line",
            "Block",
            "Start line",
            block.start_line.to_string(),
        ),
    ];
    if let Some(end_line) = block.end_line {
        rows.push(row(
            "block-end-line",
            "Block",
            "End line",
            end_line.to_string(),
        ));
    }
    if let Some(exit_status) = block.exit_status {
        rows.push(row(
            "block-exit",
            "Block",
            "Exit code",
            exit_status.to_string(),
        ));
    }
    rows.push(row(
        "block-pane",
        "Block",
        "Pane",
        pane_title.unwrap_or("—").to_owned(),
    ));
    rows
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
    use crate::chrome_palette::PaletteCommandId;
    use crate::composer::{BlockPresentationState, BlockProjection};
    use crate::shell::{ShellAction, ShellPaneSeed, ShellState, ShellTabSeed, ShellWorkspaceSeed};
    use seyal_core::{BlockId, PaneId, TabId, WorkspaceId};

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
    fn core_terminal_chrome_is_visible_by_default_and_can_recede() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        let initial = chrome.snapshot(&snap);
        assert!(initial.left_visible);
        assert!(initial.inspector_visible);
        assert!(initial.tab_strip_visible);
        chrome
            .apply(
                ChromeAction::SetShellVisibility {
                    left: false,
                    inspector: false,
                    tab_strip: false,
                },
                &snap,
            )
            .unwrap();
        let receded = chrome.snapshot(&snap);
        assert!(!receded.left_visible);
        assert!(!receded.inspector_visible);
        assert!(!receded.tab_strip_visible);
    }

    #[test]
    fn attention_popover_toggles_and_closes_on_open() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        assert!(!chrome.snapshot(&snap).attention_popover_open);
        chrome
            .apply(ChromeAction::SetAttentionPopover { open: true }, &snap)
            .unwrap();
        assert!(chrome.snapshot(&snap).attention_popover_open);
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
        chrome
            .apply(
                ChromeAction::OpenAttention {
                    id: AttentionId::new("attention-preview-tab"),
                },
                &snap,
            )
            .unwrap();
        assert!(!chrome.snapshot(&shell.snapshot()).attention_popover_open);
        assert!(chrome
            .snapshot(&shell.snapshot())
            .attention_items
            .is_empty());
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

    #[test]
    fn palette_opens_filters_runs_and_dismisses() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        assert!(!chrome.snapshot(&snap).palette_open);
        chrome
            .apply(ChromeAction::SetPaletteOpen { open: true }, &snap)
            .unwrap();
        let open = chrome.snapshot(&snap);
        assert!(open.palette_open);
        assert!(open.palette_query.is_empty());
        assert_eq!(open.palette_selected, 0);
        assert!(!open.palette_commands.is_empty());
        chrome
            .apply(
                ChromeAction::SetPaletteQuery {
                    query: "split".into(),
                },
                &snap,
            )
            .unwrap();
        let filtered = chrome.snapshot(&snap);
        assert_eq!(filtered.palette_commands.len(), 2);
        assert_eq!(filtered.palette_commands[0].id, "split-right");
        chrome
            .apply(ChromeAction::PaletteMove { delta: 1 }, &snap)
            .unwrap();
        assert_eq!(chrome.snapshot(&snap).palette_selected, 1);
        let effect = chrome.apply(ChromeAction::PaletteRun, &snap).unwrap();
        assert_eq!(effect.palette_command, Some(PaletteCommandId::SplitDown));
        let closed = chrome.snapshot(&snap);
        assert!(!closed.palette_open);
        assert!(closed.palette_query.is_empty());
        assert!(closed.palette_commands.is_empty());
    }

    #[test]
    fn palette_run_fails_closed_when_empty() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        assert_eq!(
            chrome.apply(ChromeAction::PaletteRun, &snap),
            Err(ChromeError::EmptyPaletteSelection)
        );
        chrome
            .apply(ChromeAction::SetPaletteOpen { open: true }, &snap)
            .unwrap();
        chrome
            .apply(
                ChromeAction::SetPaletteQuery {
                    query: "no-such-command".into(),
                },
                &snap,
            )
            .unwrap();
        assert!(chrome.snapshot(&snap).palette_commands.is_empty());
        assert_eq!(
            chrome.apply(ChromeAction::PaletteRun, &snap),
            Err(ChromeError::EmptyPaletteSelection)
        );
        assert!(chrome.snapshot(&snap).palette_open);
        chrome
            .apply(ChromeAction::SetPaletteOpen { open: false }, &snap)
            .unwrap();
        assert!(!chrome.snapshot(&snap).palette_open);
    }

    fn sample_blocks(pane: PaneId) -> Vec<BlockProjection> {
        vec![
            BlockProjection {
                pane,
                id: BlockId::from_bytes([0x21; 16]),
                command: "echo hi".into(),
                state: BlockPresentationState::Completed,
                start_line: 1,
                end_line: Some(1),
                exit_status: Some(0),
            },
            BlockProjection {
                pane,
                id: BlockId::from_bytes([0x22; 16]),
                command: "sleep 1".into(),
                state: BlockPresentationState::Running,
                start_line: 2,
                end_line: None,
                exit_status: None,
            },
        ]
    }

    #[test]
    fn blocks_mode_is_empty_when_focused_pane_has_no_blocks() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        chrome
            .apply(ChromeAction::SetInspectorMode(InspectorMode::Blocks), &snap)
            .unwrap();
        let visible = chrome
            .snapshot_with_blocks(&snap, &[])
            .visible_inspector_rows;
        assert!(visible.is_empty());
        assert!(visible.iter().all(|row| !row.label.contains("CPU")));
    }

    #[test]
    fn block_inspector_rows_come_from_rust_projection_only() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        let blocks = sample_blocks(snap.focused_pane);
        chrome
            .apply(ChromeAction::SetInspectorMode(InspectorMode::Blocks), &snap)
            .unwrap();
        let visible = chrome
            .snapshot_with_blocks(&snap, &blocks)
            .visible_inspector_rows;
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|row| row.section == "Block"));
        assert_eq!(visible[0].value, "echo hi · Completed");
        assert_eq!(visible[1].value, "sleep 1 · Running");
        assert!(visible.iter().all(|row| {
            !row.label.contains("CPU")
                && !row.label.contains("RSS")
                && !row.label.contains("Duration")
                && !row.value.contains("pid")
        }));
    }

    #[test]
    fn selected_block_shows_authoritative_detail_fields_only() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        let blocks = sample_blocks(snap.focused_pane);
        chrome
            .apply_with_blocks(
                ChromeAction::SelectBlock { id: blocks[0].id },
                &snap,
                &blocks,
            )
            .unwrap();
        let after = chrome.snapshot_with_blocks(&snap, &blocks);
        assert_eq!(after.inspector_mode, InspectorMode::Blocks);
        assert_eq!(after.selected_block, Some(blocks[0].id));
        let labels: Vec<_> = after
            .visible_inspector_rows
            .iter()
            .map(|row| row.label.as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "Command",
                "State",
                "Start line",
                "End line",
                "Exit code",
                "Pane"
            ]
        );
        assert_eq!(after.visible_inspector_rows[0].value, "echo hi");
        assert_eq!(after.visible_inspector_rows[1].value, "Completed");
        assert_eq!(after.visible_inspector_rows[4].value, "0");
        assert_eq!(
            chrome.apply_with_blocks(
                ChromeAction::SelectBlock {
                    id: BlockId::from_bytes([0x99; 16]),
                },
                &snap,
                &blocks,
            ),
            Err(ChromeError::UnknownBlock)
        );
        chrome
            .apply(ChromeAction::ClearBlockSelection, &snap)
            .unwrap();
        assert!(chrome
            .snapshot_with_blocks(&snap, &blocks)
            .selected_block
            .is_none());
        assert_eq!(
            chrome.snapshot_with_blocks(&snap, &blocks).inspector_mode,
            InspectorMode::Context
        );
    }

    #[test]
    fn center_surface_defaults_to_core_and_switches_to_agents() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        assert_eq!(chrome.snapshot(&snap).center_surface, CenterSurface::Core);
        chrome
            .apply(ChromeAction::SetCenterSurface(CenterSurface::Agents), &snap)
            .unwrap();
        assert_eq!(
            chrome.snapshot(&snap).center_surface,
            CenterSurface::Agents
        );
        // Mode switch does not invent agents or clear an existing selection
        // path — Agents center reuses the same authority as left inventory.
        seed_agents(&mut chrome, &snap);
        chrome
            .apply(
                ChromeAction::SelectAgent {
                    id: AgentId::new("agent-claude"),
                },
                &snap,
            )
            .unwrap();
        chrome
            .apply(ChromeAction::SetCenterSurface(CenterSurface::Core), &snap)
            .unwrap();
        let after = chrome.snapshot(&snap);
        assert_eq!(after.center_surface, CenterSurface::Core);
        assert_eq!(after.selected_agent, Some(AgentId::new("agent-claude")));
        assert_eq!(after.agents.len(), 2);
    }

    #[test]
    fn agents_center_selection_fails_closed_for_unknown_agent() {
        let shell = seed_shell();
        let snap = shell.snapshot();
        let mut chrome = ChromeState::new();
        seed_agents(&mut chrome, &snap);
        chrome
            .apply(ChromeAction::SetCenterSurface(CenterSurface::Agents), &snap)
            .unwrap();
        assert_eq!(
            chrome.apply(
                ChromeAction::SelectAgent {
                    id: AgentId::new("missing-agent"),
                },
                &snap,
            ),
            Err(ChromeError::UnknownAgent)
        );
        let after = chrome.snapshot(&snap);
        assert_eq!(after.center_surface, CenterSurface::Agents);
        assert!(after.selected_agent.is_none());
        assert_eq!(after.last_error, Some(ChromeError::UnknownAgent));
    }
}
