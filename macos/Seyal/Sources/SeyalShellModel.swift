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
}

#if DEBUG
enum SeyalShellPreviewData {
    /// Deterministic fixture data used only by the explicit --ui-shell-preview path.
    /// It is never a substitute for Runtime-backed state.
    static let snapshot = SeyalShellSnapshot(
        workspaces: [
            .init(id: "workspace-seyal", name: "Seyal", detail: "~/Developer/seyal", attention: false),
            .init(id: "workspace-ops", name: "Production Ops", detail: "remote context", attention: true),
        ],
        activeWorkspaceID: "workspace-seyal",
        tabs: [
            .init(id: "tab-terminal", title: "Terminal", attention: false, paneCount: 1),
            .init(id: "tab-build", title: "Build", attention: true, paneCount: 2),
            .init(id: "tab-agent", title: "Agent", attention: false, paneCount: 1),
        ],
        activeTabID: "tab-terminal",
        agents: [
            .init(id: "agent-claude", name: "Claude Code", state: .running),
            .init(id: "agent-codex", name: "Codex", state: .attention),
        ],
        inspectorRows: [
            .init(id: "workspace", label: "Workspace", value: "Seyal"),
            .init(id: "pane", label: "Pane", value: "Terminal 1"),
            .init(id: "shell", label: "Shell", value: "zsh"),
            .init(id: "cwd", label: "Working directory", value: "~/Developer/seyal"),
        ],
        attentionItems: [
            .init(id: "attention-agent", title: "Agent needs attention", detail: "Review requested in the active workspace"),
            .init(id: "attention-build", title: "Build finished", detail: "Background build completed"),
        ]
    )

    static let blocks: [BlockPresentation] = [
        .init(
            id: "block-1",
            command: "git status --short",
            state: .completed,
            elapsed: "84 ms",
            timestamp: "07:42",
            isSelected: false
        ),
        .init(
            id: "block-2",
            command: "make test",
            state: .running,
            elapsed: "12.4 s",
            timestamp: "07:43",
            isSelected: true
        ),
    ]

    static let terminalLines: [[String]] = [
        [" M docs/architecture/ui/M001-CORE-TERMINAL-REFERENCE-SCREEN.md", "?? macos/Seyal/Sources/"],
        ["running 18 tests", "test terminal_execution_feeds_the_single_authoritative_terminal_state ... ok", "test alternate_screen_preserves_primary_and_is_discarded_on_leave ... ok", "test result: ok. 18 passed; 0 failed"],
    ]
}
#endif
