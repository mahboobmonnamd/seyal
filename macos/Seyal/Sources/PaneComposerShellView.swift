import AppKit

@MainActor
final class PaneComposerShellView: NSView, NSTextViewDelegate {
    enum Mode {
        case available
        case busy(process: String)
        case hiddenForTUI
    }

    private let mode: Mode
    private let draft: String
    private let accessibilityID: String?
    private let onFocus: (() -> Void)?
    private let onDraftChange: ((String) -> Void)?
    private weak var editor: NSTextView?

    init(
        mode: Mode,
        draft: String,
        accessibilityID: String? = nil,
        onFocus: (() -> Void)? = nil,
        onDraftChange: ((String) -> Void)? = nil
    ) {
        self.mode = mode
        self.draft = draft
        self.accessibilityID = accessibilityID
        self.onFocus = onFocus
        self.onDraftChange = onDraftChange
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
        prompt.setContentHuggingPriority(.required, for: .horizontal)

        let editor = NSTextView(frame: .zero)
        editor.translatesAutoresizingMaskIntoConstraints = false
        editor.isRichText = false
        editor.importsGraphics = false
        editor.drawsBackground = false
        editor.font = SeyalDesignTokens.Typography.command
        editor.textColor = SeyalDesignTokens.Palette.textPrimary
        editor.insertionPointColor = SeyalDesignTokens.Palette.focus
        editor.string = draft
        editor.delegate = self
        editor.isHorizontallyResizable = false
        editor.isVerticallyResizable = true
        editor.textContainerInset = NSSize(width: 0, height: 5)
        editor.textContainer?.widthTracksTextView = true
        editor.textContainer?.lineFragmentPadding = 0
        if let accessibilityID {
            editor.setAccessibilityIdentifier(accessibilityID)
        }
        editor.setAccessibilityLabel("Pane command composer")
        self.editor = editor

        let hint = NSTextField(labelWithString: "Shift+Return newline")
        hint.font = SeyalDesignTokens.Typography.metadata
        hint.textColor = SeyalDesignTokens.Palette.textTertiary
        hint.alignment = .right
        hint.setContentHuggingPriority(.required, for: .horizontal)

        let row = NSStackView(views: [prompt, editor, hint])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 9
        row.translatesAutoresizingMaskIntoConstraints = false
        addSubview(row)

        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            row.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            row.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            row.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
            editor.heightAnchor.constraint(greaterThanOrEqualToConstant: 38),
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

    func textDidBeginEditing(_ notification: Notification) {
        onFocus?()
    }

    func textDidChange(_ notification: Notification) {
        guard let editor else { return }
        onDraftChange?(editor.string)
    }
}
