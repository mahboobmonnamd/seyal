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
        SeyalDesignTokens.configureRoundedPanel(self, radius: 9)

        let previewDraft = draft.replacingOccurrences(of: "\\n", with: "\n")
        let draftField = NSTextField(wrappingLabelWithString: previewDraft)
        draftField.font = SeyalDesignTokens.Typography.command
        draftField.textColor = SeyalDesignTokens.Palette.textPrimary
        draftField.maximumNumberOfLines = 4
        draftField.lineBreakMode = .byWordWrapping
        draftField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let hint = NSTextField(labelWithString: "Pane composer preview · Shift+Return newline · execute binding comes with native input")
        hint.font = SeyalDesignTokens.Typography.metadata
        hint.textColor = SeyalDesignTokens.Palette.textTertiary
        hint.lineBreakMode = .byTruncatingTail

        let stack = NSStackView(views: [draftField, hint])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 6
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 10),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -9),
            heightAnchor.constraint(greaterThanOrEqualToConstant: SeyalDesignTokens.Layout.composerMinHeight),
            heightAnchor.constraint(lessThanOrEqualToConstant: SeyalDesignTokens.Layout.composerMaxPreviewHeight),
        ])
    }

    private func buildBusyState(process: String) {
        SeyalDesignTokens.configureRoundedPanel(self, radius: 9)

        let status = NSTextField(labelWithString: "Foreground process running: \(process)")
        status.font = SeyalDesignTokens.Typography.bodyEmphasized
        status.textColor = SeyalDesignTokens.Palette.textSecondary

        status.translatesAutoresizingMaskIntoConstraints = false
        addSubview(status)

        NSLayoutConstraint.activate([
            status.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            status.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -12),
            status.centerYAnchor.constraint(equalTo: centerYAnchor),
            heightAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.composerMinHeight),
        ])
    }
}
