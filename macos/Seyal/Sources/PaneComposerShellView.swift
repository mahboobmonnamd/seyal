import AppKit

@MainActor
final class PaneComposerShellView: NSView {
    enum Mode {
        case available
        case busy(process: String)
        case hiddenForTUI
    }

    private let mode: Mode
    private let draft: String

    init(mode: Mode, draft: String) {
        self.mode = mode
        self.draft = draft
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        buildUI()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("PaneComposerShellView is programmatic")
    }

    private func buildUI() {
        switch mode {
        case .hiddenForTUI:
            isHidden = true
            return
        case .available:
            buildAvailableComposer()
        case let .busy(process):
            buildBusyState(process: process)
        }
    }

    private func buildAvailableComposer() {
        SeyalDesignTokens.configureRoundedPanel(
            self,
            radius: 10,
            background: SeyalDesignTokens.Palette.elevatedBackground
        )
        layer?.borderColor = SeyalDesignTokens.Palette.focus.cgColor

        let prompt = NSTextField(labelWithString: "›")
        prompt.font = NSFont.monospacedSystemFont(ofSize: 18, weight: .semibold)
        prompt.textColor = SeyalDesignTokens.Palette.focus

        let previewDraft = draft.replacingOccurrences(of: "\\n", with: "\n")
        let text = previewDraft.isEmpty ? "Type a command, @ agent, / action…" : previewDraft
        let draftField = NSTextField(wrappingLabelWithString: text)
        draftField.font = SeyalDesignTokens.Typography.command
        draftField.textColor = previewDraft.isEmpty
            ? SeyalDesignTokens.Palette.textTertiary
            : SeyalDesignTokens.Palette.textPrimary
        draftField.maximumNumberOfLines = 4
        draftField.lineBreakMode = .byWordWrapping
        draftField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let execute = NSTextField(labelWithString: "↵")
        execute.font = NSFont.systemFont(ofSize: 15, weight: .semibold)
        execute.textColor = SeyalDesignTokens.Palette.textPrimary
        execute.alignment = .center
        execute.wantsLayer = true
        execute.layer?.cornerRadius = 7
        execute.layer?.backgroundColor = SeyalDesignTokens.Palette.focus.cgColor
        execute.translatesAutoresizingMaskIntoConstraints = false
        execute.widthAnchor.constraint(equalToConstant: 30).isActive = true
        execute.heightAnchor.constraint(equalToConstant: 26).isActive = true
        execute.toolTip = "Execute command"

        let inputRow = NSStackView(views: [prompt, draftField, execute])
        inputRow.orientation = .horizontal
        inputRow.alignment = .centerY
        inputRow.spacing = 9
        inputRow.translatesAutoresizingMaskIntoConstraints = false

        let history = NSTextField(labelWithString: "⌃R history")
        history.font = SeyalDesignTokens.Typography.metadata
        history.textColor = SeyalDesignTokens.Palette.textTertiary

        let newline = NSTextField(labelWithString: "⇧↩ newline")
        newline.font = SeyalDesignTokens.Typography.metadata
        newline.textColor = SeyalDesignTokens.Palette.textTertiary

        let helper = NSStackView(views: [history, newline])
        helper.orientation = .horizontal
        helper.alignment = .centerY
        helper.spacing = 12

        let stack = NSStackView(views: [inputRow, helper])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 7
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 10),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
            inputRow.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            inputRow.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            heightAnchor.constraint(greaterThanOrEqualToConstant: SeyalDesignTokens.Layout.composerMinHeight),
            heightAnchor.constraint(lessThanOrEqualToConstant: SeyalDesignTokens.Layout.composerMaxPreviewHeight),
        ])
    }

    private func buildBusyState(process: String) {
        SeyalDesignTokens.configureRoundedPanel(
            self,
            radius: 10,
            background: SeyalDesignTokens.Palette.elevatedBackground
        )

        let dot = NSTextField(labelWithString: "●")
        dot.font = NSFont.systemFont(ofSize: 9, weight: .bold)
        dot.textColor = SeyalDesignTokens.Palette.warning

        let status = NSTextField(labelWithString: "\(process) is using this shell · open another Pane to run in parallel")
        status.font = SeyalDesignTokens.Typography.bodyEmphasized
        status.textColor = SeyalDesignTokens.Palette.textSecondary
        status.lineBreakMode = .byTruncatingTail

        let row = NSStackView(views: [dot, status])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 8
        row.translatesAutoresizingMaskIntoConstraints = false
        addSubview(row)

        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            row.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -12),
            row.centerYAnchor.constraint(equalTo: centerYAnchor),
            heightAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.composerMinHeight),
        ])
    }
}
