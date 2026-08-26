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
enum SeyalShellPreviewData {
    /// Deterministic fixture data used only by the explicit --ui-shell-preview path.
    /// It is never a substitute for Runtime-backed state.
    static let snapshot = SeyalShellSnapshot(
        workspaces: [
            .init(
                id: "workspace-seyal",
                name: "Seyal OSS",
                detail: "~/Projects/seyal",
                attention: false,
                tabCount: 4
            ),
            .init(
                id: "workspace-payments",
                name: "Payments Platform",
                detail: "~/Projects/payments",
                attention: false,
                tabCount: 6
            ),
            .init(
                id: "workspace-ops",
                name: "Infra Operations",
                detail: "~/Ops/infra",
                attention: true,
                tabCount: 3
            ),
        ],
        activeWorkspaceID: "workspace-seyal",
        tabs: [
            .init(id: "tab-terminal", title: "Core Terminal", attention: false, paneCount: 1),
            .init(id: "tab-agent", title: "Agent Development", attention: false, paneCount: 1),
            .init(id: "tab-logs", title: "Logs & Monitoring", attention: true, paneCount: 2),
            .init(id: "tab-review", title: "PR Review", attention: false, paneCount: 1),
        ],
        activeTabID: "tab-terminal",
        agents: [
            .init(id: "agent-claude", name: "Claude Code", state: .running),
            .init(id: "agent-codex", name: "Codex", state: .attention),
            .init(id: "agent-opencode", name: "OpenCode", state: .idle),
        ],
        inspectorRows: [
            .init(id: "workspace-name", section: "Workspace", label: "Name", value: "Seyal OSS"),
            .init(id: "workspace-path", section: "Workspace", label: "Path", value: "~/Projects/seyal"),
            .init(id: "workspace-branch", section: "Workspace", label: "Branch", value: "main"),
            .init(id: "tab-name", section: "Tab", label: "Name", value: "Core Terminal"),
            .init(id: "tab-layout", section: "Tab", label: "Layout", value: "Single pane"),
            .init(id: "pane-name", section: "Active Pane", label: "Pane", value: "Pane 1 · Focused"),
            .init(id: "pane-shell", section: "Active Pane", label: "Shell", value: "zsh"),
            .init(id: "pane-cwd", section: "Active Pane", label: "CWD", value: "~/Projects/seyal"),
            .init(id: "runtime-state", section: "Runtime", label: "Execution", value: "Attached · Idle"),
            .init(id: "runtime-link", section: "Runtime", label: "Connection", value: "Local"),
        ],
        attentionItems: [
            .init(
                id: "attention-agent",
                title: "Agent needs attention",
                detail: "Codex is waiting for review in Seyal OSS"
            ),
            .init(
                id: "attention-logs",
                title: "Logs tab needs attention",
                detail: "A background task reported a failure"
            ),
        ]
    )

    static let blocks: [BlockPresentation] = [
        .init(
            id: "block-git",
            command: "git status",
            state: .completed,
            elapsed: "84 ms",
            timestamp: "Today 7:28 PM",
            isSelected: false,
            actions: ["Copy", "Rerun", "Pin", "Expand"]
        ),
        .init(
            id: "block-test",
            command: "cargo test -p seyal-terminal",
            state: .completed,
            elapsed: "1.8 s",
            timestamp: "Today 7:27 PM",
            isSelected: true,
            actions: ["Copy", "Rerun", "Pin", "Expand"]
        ),
        .init(
            id: "block-kubectl",
            command: "kubectl get pods -n seyal",
            state: .completed,
            elapsed: "312 ms",
            timestamp: "Today 7:23 PM",
            isSelected: false,
            actions: ["Copy", "Rerun", "Pin", "Expand"]
        ),
    ]

    static let terminalLines: [[String]] = [
        [
            "On branch main",
            "Your branch is up to date with 'origin/main'.",
            "",
            "Changes not staged for commit:",
            "  modified:   macos/Seyal/Sources/SeyalShellView.swift",
            "  modified:   macos/Seyal/Sources/BlockView.swift",
        ],
        [
            "running 18 tests",
            "test parser::osc_title ... ok",
            "test terminal_state::alternate_screen ... ok",
            "test terminal_state::scrollback_damage ... ok",
            "test result: ok. 18 passed; 0 failed",
        ],
        [
            "NAME                              READY   STATUS    RESTARTS   AGE",
            "seyal-api-7d8f6d7b7c-2k9m4       1/1     Running   0          12m",
            "seyal-worker-6f7b8c9d9f-x7k3n    1/1     Running   0          12m",
            "seyal-db-0                        1/1     Running   0          3h12m",
        ],
    ]
}
#endif
