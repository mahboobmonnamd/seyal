//! Built-in global command-palette catalog and query filtering.
//!
//! Rust owns the authoritative open/query/selection state (via [`crate::chrome`])
//! and the static command list. Hosts project the filtered snapshot and never
//! invent a parallel command registry. Filtering is case-insensitive substring
//! match only — no fuzzy ranking (#932).

/// Stable product identity for one built-in palette command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaletteCommandId {
    CreateTab,
    SplitRight,
    SplitDown,
    ToggleLeft,
    ToggleInspector,
    ToggleAttentionPopover,
    ShowWorkspaces,
    ShowTabs,
    ShowCoreCenter,
    ShowAgentsCenter,
    InspectorContext,
    InspectorWorkspace,
    InspectorTab,
    InspectorPane,
    InspectorBlocks,
}

impl PaletteCommandId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateTab => "create-tab",
            Self::SplitRight => "split-right",
            Self::SplitDown => "split-down",
            Self::ToggleLeft => "toggle-left",
            Self::ToggleInspector => "toggle-inspector",
            Self::ToggleAttentionPopover => "toggle-attention",
            Self::ShowWorkspaces => "show-workspaces",
            Self::ShowTabs => "show-tabs",
            Self::ShowCoreCenter => "show-core-center",
            Self::ShowAgentsCenter => "show-agents-center",
            Self::InspectorContext => "inspector-context",
            Self::InspectorWorkspace => "inspector-workspace",
            Self::InspectorTab => "inspector-tab",
            Self::InspectorPane => "inspector-pane",
            Self::InspectorBlocks => "inspector-blocks",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::CreateTab => "Create Tab",
            Self::SplitRight => "Split Right",
            Self::SplitDown => "Split Down",
            Self::ToggleLeft => "Toggle Left Panel",
            Self::ToggleInspector => "Toggle Inspector",
            Self::ToggleAttentionPopover => "Toggle Attention Popover",
            Self::ShowWorkspaces => "Show Workspaces",
            Self::ShowTabs => "Show Tabs",
            Self::ShowCoreCenter => "Show Core Terminal",
            Self::ShowAgentsCenter => "Show Agents",
            Self::InspectorContext => "Inspector: Context",
            Self::InspectorWorkspace => "Inspector: Workspace",
            Self::InspectorTab => "Inspector: Tab",
            Self::InspectorPane => "Inspector: Pane",
            Self::InspectorBlocks => "Inspector: Blocks",
        }
    }

    pub fn from_str_id(id: &str) -> Option<Self> {
        BUILTIN.iter().find(|cmd| cmd.as_str() == id).copied()
    }
}

const BUILTIN: &[PaletteCommandId] = &[
    PaletteCommandId::CreateTab,
    PaletteCommandId::SplitRight,
    PaletteCommandId::SplitDown,
    PaletteCommandId::ToggleLeft,
    PaletteCommandId::ToggleInspector,
    PaletteCommandId::ToggleAttentionPopover,
    PaletteCommandId::ShowWorkspaces,
    PaletteCommandId::ShowTabs,
    PaletteCommandId::ShowCoreCenter,
    PaletteCommandId::ShowAgentsCenter,
    PaletteCommandId::InspectorContext,
    PaletteCommandId::InspectorWorkspace,
    PaletteCommandId::InspectorTab,
    PaletteCommandId::InspectorPane,
    PaletteCommandId::InspectorBlocks,
];

/// Host-facing filtered palette row. Identities are catalog ids, not Runtime ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteCommandRow {
    pub id: String,
    pub title: String,
}

/// Case-insensitive substring filter over the built-in catalog.
pub fn filter_commands(query: &str) -> Vec<PaletteCommandRow> {
    let needle = query.trim().to_lowercase();
    BUILTIN
        .iter()
        .copied()
        .filter(|cmd| {
            if needle.is_empty() {
                return true;
            }
            cmd.title().to_lowercase().contains(&needle)
                || cmd.as_str().to_lowercase().contains(&needle)
        })
        .map(|cmd| PaletteCommandRow {
            id: cmd.as_str().to_owned(),
            title: cmd.title().to_owned(),
        })
        .collect()
}

/// Clamp selection into `[0, len)` or `0` when empty (fail-closed empty run).
pub fn clamp_selected(selected: usize, filtered_len: usize) -> usize {
    if filtered_len == 0 {
        0
    } else {
        selected.min(filtered_len - 1)
    }
}

/// Move selection by `delta` with wrap-around. Empty lists stay at `0`.
pub fn move_selected(selected: usize, delta: i32, filtered_len: usize) -> usize {
    if filtered_len == 0 {
        return 0;
    }
    let len = filtered_len as i32;
    let mut next = selected as i32 + delta;
    next %= len;
    if next < 0 {
        next += len;
    }
    next as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_lists_all_builtin_commands() {
        let rows = filter_commands("");
        assert_eq!(rows.len(), BUILTIN.len());
        assert_eq!(rows[0].id, "create-tab");
        assert_eq!(rows[0].title, "Create Tab");
    }

    #[test]
    fn filter_is_case_insensitive_substring() {
        let rows = filter_commands("SpLiT");
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["split-right", "split-down"]);
        assert!(filter_commands("zzzz-missing").is_empty());
    }

    #[test]
    fn selection_helpers_fail_closed_on_empty() {
        assert_eq!(clamp_selected(3, 0), 0);
        assert_eq!(move_selected(0, 1, 0), 0);
        assert_eq!(clamp_selected(9, 3), 2);
        assert_eq!(move_selected(0, -1, 3), 2);
        assert_eq!(move_selected(2, 1, 3), 0);
    }
}
