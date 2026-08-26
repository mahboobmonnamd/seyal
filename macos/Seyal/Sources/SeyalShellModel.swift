import Foundation

/// Read-only presentation data for the macOS shell. This model is intentionally
/// derived UI state: it never owns PTYs, VT state, terminal grids, or execution
/// lifecycle authority.
struct SeyalShellSnapshot: Sendable {
    struct Workspace: Sendable, Identifiable {
        let id: String
        let name: String
        let detail: String?
        let attention: Bool
        let tabCount: Int
    }

    struct Tab: Sendable, Identifiable {
        let id: String
        let title: String
        let attention: Bool
        let paneCount: Int
    }

    struct Agent: Sendable, Identifiable {
        enum State: String, Sendable {
            case running = "Running"
            case waiting = "Waiting"
            case attention = "Attention"
            case idle = "Idle"
        }

        let id: String
        let name: String
        let state: State
    }

    struct InspectorRow: Sendable, Identifiable {
        let id: String
        let section: String
        let label: String
        let value: String
    }

    struct AttentionItem: Sendable, Identifiable {
        let id: String
        let title: String
        let detail: String
        let workspaceID: String?
        let tabID: String?
        let agentID: String?
    }

    let workspaces: [Workspace]
    let activeWorkspaceID: String
    let tabs: [Tab]
    let activeTabID: String
    let agents: [Agent]
    let inspectorRows: [InspectorRow]
    let attentionItems: [AttentionItem]
}

enum BlockPresentationState: String, Sendable {
    case running = "Running"
    case completed = "Completed"
    case failed = "Failed"
}

struct BlockPresentation: Sendable, Identifiable {
    let id: String
    let command: String
    let state: BlockPresentationState
    let elapsed: String
    let timestamp: String?
    let isSelected: Bool
    let actions: [String]
}

#if DEBUG
/// Interactive state used only by the explicit pre-Pass-6 UI preview.
///
/// This state intentionally models navigation/focus/layout/draft behavior only.
/// It is not Runtime state and never represents a PTY, VT grid, process, shell,
/// execution lifecycle, or resource telemetry.
@MainActor
final class SeyalShellPreviewState {
    enum SplitAxis: String, Sendable {
        case right
        case down
    }

    indirect enum PaneTree: Sendable {
        case pane(String)
        case split(axis: SplitAxis, first: PaneTree, second: PaneTree)
    }

    final class Pane {
        let id: String
        let title: String
        var draft: String

        init(id: String, title: String, draft: String = "") {
            self.id = id
            self.title = title
            self.draft = draft
        }
    }

    final class Tab {
        let id: String
        var title: String
        var attention: Bool
        var panes: [String: Pane]
        var root: PaneTree
        var focusedPaneID: String

        init(id: String, title: String, attention: Bool = false, pane: Pane) {
            self.id = id
            self.title = title
            self.attention = attention
            panes = [pane.id: pane]
            root = .pane(pane.id)
            focusedPaneID = pane.id
        }

        var paneCount: Int { panes.count }

        var layoutDescription: String {
            switch root {
            case .pane:
                "Single pane"
            case let .split(axis, _, _):
                axis == .right ? "Split right" : "Split down"
            }
        }
    }

    final class Workspace {
        let id: String
        let name: String
        let detail: String?
        var attention: Bool
        var tabs: [Tab]
        var activeTabID: String
        var agents: [SeyalShellSnapshot.Agent]

        init(
            id: String,
            name: String,
            detail: String?,
            attention: Bool = false,
            tabs: [Tab],
            activeTabID: String,
            agents: [SeyalShellSnapshot.Agent] = []
        ) {
            self.id = id
            self.name = name
            self.detail = detail
            self.attention = attention
            self.tabs = tabs
            self.activeTabID = activeTabID
            self.agents = agents
        }
    }

    private(set) var workspaces: [Workspace]
    private(set) var activeWorkspaceID: String
    private(set) var attentionItems: [SeyalShellSnapshot.AttentionItem]
    private(set) var selectedAgentID: String?

    private var nextTabOrdinal = 5
    private var nextPaneOrdinal = 2

    init(
        workspaces: [Workspace],
        activeWorkspaceID: String,
        attentionItems: [SeyalShellSnapshot.AttentionItem] = []
    ) {
        precondition(!workspaces.isEmpty, "Preview requires at least one Workspace")
        self.workspaces = workspaces
        self.activeWorkspaceID = activeWorkspaceID
        self.attentionItems = attentionItems
    }

    static func makeDefault(includeTestAttention: Bool = false) -> SeyalShellPreviewState {
        func terminalTab(id: String, title: String, paneID: String, attention: Bool = false) -> Tab {
            Tab(
                id: id,
                title: title,
                attention: attention,
                pane: Pane(id: paneID, title: "Pane 1")
            )
        }

        let seyalTabs = [
            terminalTab(id: "tab-terminal", title: "Core Terminal", paneID: "pane-1"),
            terminalTab(id: "tab-agent", title: "Agent Development", paneID: "pane-agent"),
            terminalTab(id: "tab-logs", title: "Logs & Monitoring", paneID: "pane-logs"),
            terminalTab(id: "tab-review", title: "PR Review", paneID: "pane-review"),
        ]

        let labTabs = [
            terminalTab(id: "tab-lab-terminal", title: "Terminal", paneID: "pane-lab"),
        ]

        let workspaces = [
            Workspace(
                id: "workspace-seyal",
                name: "Seyal OSS",
                detail: "~/Projects/seyal",
                tabs: seyalTabs,
                activeTabID: "tab-terminal"
            ),
            Workspace(
                id: "workspace-lab",
                name: "Personal Lab",
                detail: "~/Lab",
                tabs: labTabs,
                activeTabID: "tab-lab-terminal"
            ),
        ]

        let testAttention: [SeyalShellSnapshot.AttentionItem] = includeTestAttention
            ? [
                .init(
                    id: "attention-preview-tab",
                    title: "Preview attention item",
                    detail: "Open Agent Development",
                    workspaceID: "workspace-seyal",
                    tabID: "tab-agent",
                    agentID: nil
                ),
            ]
            : []

        return SeyalShellPreviewState(
            workspaces: workspaces,
            activeWorkspaceID: "workspace-seyal",
            attentionItems: testAttention
        )
    }

    var activeWorkspace: Workspace {
        guard let workspace = workspaces.first(where: { $0.id == activeWorkspaceID }) else {
            preconditionFailure("Active preview Workspace must exist")
        }
        return workspace
    }

    var activeTab: Tab {
        let workspace = activeWorkspace
        guard let tab = workspace.tabs.first(where: { $0.id == workspace.activeTabID }) else {
            preconditionFailure("Active preview Tab must exist")
        }
        return tab
    }

    var focusedPane: Pane {
        let tab = activeTab
        guard let pane = tab.panes[tab.focusedPaneID] else {
            preconditionFailure("Focused preview Pane must exist")
        }
        return pane
    }

    var snapshot: SeyalShellSnapshot {
        let workspace = activeWorkspace
        let tab = activeTab
        let inspectorRows: [SeyalShellSnapshot.InspectorRow]

        if let selectedAgentID,
           let agent = workspace.agents.first(where: { $0.id == selectedAgentID }) {
            inspectorRows = [
                .init(id: "agent-name", section: "Agent", label: "Name", value: agent.name),
                .init(id: "agent-state", section: "Agent", label: "State", value: agent.state.rawValue),
                .init(id: "agent-workspace", section: "Workspace", label: "Name", value: workspace.name),
            ]
        } else {
            inspectorRows = [
                .init(id: "workspace-name", section: "Workspace", label: "Name", value: workspace.name),
                .init(id: "workspace-path", section: "Workspace", label: "Path", value: workspace.detail ?? "—"),
                .init(id: "tab-name", section: "Tab", label: "Name", value: tab.title),
                .init(id: "tab-panes", section: "Tab", label: "Panes", value: String(tab.paneCount)),
                .init(id: "tab-layout", section: "Tab", label: "Layout", value: tab.layoutDescription),
                .init(id: "pane-name", section: "Active Pane", label: "Pane", value: focusedPane.title),
                .init(id: "pane-focus", section: "Active Pane", label: "Focus", value: "Focused"),
            ]
        }

        return SeyalShellSnapshot(
            workspaces: workspaces.map {
                .init(
                    id: $0.id,
                    name: $0.name,
                    detail: $0.detail,
                    attention: $0.attention,
                    tabCount: $0.tabs.count
                )
            },
            activeWorkspaceID: activeWorkspaceID,
            tabs: workspace.tabs.map {
                .init(
                    id: $0.id,
                    title: $0.title,
                    attention: $0.attention,
                    paneCount: $0.paneCount
                )
            },
            activeTabID: workspace.activeTabID,
            agents: workspace.agents,
            inspectorRows: inspectorRows,
            attentionItems: attentionItems
        )
    }

    func selectWorkspace(id: String) {
        guard workspaces.contains(where: { $0.id == id }) else { return }
        activeWorkspaceID = id
        selectedAgentID = nil
    }

    func selectTab(id: String) {
        let workspace = activeWorkspace
        guard workspace.tabs.contains(where: { $0.id == id }) else { return }
        workspace.activeTabID = id
        selectedAgentID = nil
    }

    func selectAgent(id: String) {
        guard activeWorkspace.agents.contains(where: { $0.id == id }) else { return }
        selectedAgentID = id
    }

    @discardableResult
    func createTab() -> Tab {
        let pane = Pane(id: "pane-new-\(nextPaneOrdinal)", title: "Pane 1")
        nextPaneOrdinal += 1
        let tab = Tab(
            id: "tab-new-\(nextTabOrdinal)",
            title: "Terminal \(nextTabOrdinal)",
            pane: pane
        )
        nextTabOrdinal += 1
        activeWorkspace.tabs.append(tab)
        activeWorkspace.activeTabID = tab.id
        selectedAgentID = nil
        return tab
    }

    func closeTab(id: String) {
        let workspace = activeWorkspace
        guard workspace.tabs.count > 1,
              let index = workspace.tabs.firstIndex(where: { $0.id == id }) else {
            return
        }

        workspace.tabs.remove(at: index)
        if workspace.activeTabID == id {
            let replacementIndex = min(index, workspace.tabs.count - 1)
            workspace.activeTabID = workspace.tabs[replacementIndex].id
        }
        selectedAgentID = nil
    }

    @discardableResult
    func splitFocusedPane(axis: SplitAxis) -> Pane {
        let tab = activeTab
        let currentPaneID = tab.focusedPaneID
        let pane = Pane(id: "pane-new-\(nextPaneOrdinal)", title: "Pane \(tab.paneCount + 1)")
        nextPaneOrdinal += 1

        tab.panes[pane.id] = pane
        tab.root = replacingPane(
            currentPaneID,
            in: tab.root,
            with: .split(axis: axis, first: .pane(currentPaneID), second: .pane(pane.id))
        )
        tab.focusedPaneID = pane.id
        selectedAgentID = nil
        return pane
    }

    func focusPane(id: String) {
        let tab = activeTab
        guard tab.panes[id] != nil else { return }
        tab.focusedPaneID = id
        selectedAgentID = nil
    }

    func updateDraft(_ draft: String, paneID: String) {
        guard let pane = activeTab.panes[paneID] else { return }
        pane.draft = draft
    }

    func openAttentionItem(id: String) {
        guard let index = attentionItems.firstIndex(where: { $0.id == id }) else { return }
        let item = attentionItems[index]
        if let workspaceID = item.workspaceID {
            selectWorkspace(id: workspaceID)
        }
        if let tabID = item.tabID {
            selectTab(id: tabID)
        }
        if let agentID = item.agentID {
            selectAgent(id: agentID)
        }
        attentionItems.remove(at: index)
    }

    private func replacingPane(
        _ targetID: String,
        in node: PaneTree,
        with replacement: PaneTree
    ) -> PaneTree {
        switch node {
        case let .pane(id):
            return id == targetID ? replacement : node
        case let .split(axis, first, second):
            return .split(
                axis: axis,
                first: replacingPane(targetID, in: first, with: replacement),
                second: replacingPane(targetID, in: second, with: replacement)
            )
        }
    }
}
#endif
