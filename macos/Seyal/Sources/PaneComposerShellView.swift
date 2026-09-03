import AppKit

@MainActor
private final class PaneComposerTextView: NSTextView {
    var onSubmit: ((String) -> Bool)?

    override func doCommand(by selector: Selector) {
        let isReturn = Self.isReturnSelector(selector)
        let isShiftReturn = NSApp.currentEvent?.modifierFlags.contains(.shift) == true
        let command = string.trimmingCharacters(in: .whitespacesAndNewlines)
        if isReturn && !isShiftReturn && !command.isEmpty {
            _ = onSubmit?(command)
            return
        }
        super.doCommand(by: selector)
    }

    static func isReturnSelector(_ selector: Selector) -> Bool {
        selector == #selector(NSResponder.insertNewline(_:))
            || selector == #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:))
    }
}

@MainActor
final class PaneComposerShellView: NSView, NSTextViewDelegate {
    enum Mode {
        case available
        case busy(process: String)
        case hiddenForTUI
    }

    private let mode: Mode
    private let draft: String
    private var visual: SeyalResolvedVisualConfiguration
    private let accessibilityID: String?
    private let onFocus: (() -> Void)?
    private let onDraftChange: ((String) -> Void)?
    private let onSubmit: ((String) -> Bool)?
    private weak var editor: NSTextView?

    init(
        mode: Mode,
        draft: String,
        visual: SeyalResolvedVisualConfiguration,
        accessibilityID: String? = nil,
        onFocus: (() -> Void)? = nil,
        onDraftChange: ((String) -> Void)? = nil,
        onSubmit: ((String) -> Bool)? = nil
    ) {
        self.mode = mode
        self.draft = draft
        self.visual = visual
        self.accessibilityID = accessibilityID
        self.onFocus = onFocus
        self.onDraftChange = onDraftChange
        self.onSubmit = onSubmit
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        buildUI()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("PaneComposerShellView is programmatic")
    }

    func applyVisual(_ visual: SeyalResolvedVisualConfiguration) {
        self.visual = visual
        let depth: SeyalDepthLevel
        switch mode {
        case .available: depth = .activeUtility
        case .busy, .hiddenForTUI: depth = .recededUtility
        }
        SeyalMaterialPresenter.apply(
            depth,
            to: self,
            visual: visual,
            cornerRadius: visual.metrics.composerCornerRadius
        )
        editor?.font = visual.typography[.composer]
        editor?.textColor = visual.colors.ns(.textPrimary)
        editor?.insertionPointColor = visual.colors.ns(.focus)
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
        SeyalMaterialPresenter.apply(
            .activeUtility,
            to: self,
            visual: visual,
            cornerRadius: visual.metrics.composerCornerRadius
        )

        let prompt = NSTextField(labelWithString: "›")
        prompt.font = visual.typography[.composer]
        prompt.textColor = visual.colors.ns(.focus)
        prompt.setContentHuggingPriority(.required, for: .horizontal)

        let editor = PaneComposerTextView(frame: .zero)
        editor.translatesAutoresizingMaskIntoConstraints = false
        editor.isRichText = false
        editor.importsGraphics = false
        editor.drawsBackground = false
        editor.font = visual.typography[.composer]
        editor.textColor = visual.colors.ns(.textPrimary)
        editor.insertionPointColor = visual.colors.ns(.focus)
        editor.string = draft
        editor.delegate = self
        editor.onSubmit = { [weak self, weak editor] command in
            guard self?.onSubmit?(command) == true else { return false }
            editor?.string = ""
            return true
        }
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
        hint.font = visual.typography[.metadata]
        hint.textColor = visual.colors.ns(.textMuted)
        hint.alignment = .right
        hint.setContentHuggingPriority(.required, for: .horizontal)

        let row = NSStackView(views: [prompt, editor, hint])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = visual.metrics.sm + 1
        row.translatesAutoresizingMaskIntoConstraints = false
        addSubview(row)

        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(
                equalTo: leadingAnchor, constant: visual.metrics.composerInsetHorizontal),
            row.trailingAnchor.constraint(
                equalTo: trailingAnchor, constant: -visual.metrics.composerInsetHorizontal),
            row.topAnchor.constraint(
                equalTo: topAnchor, constant: visual.metrics.composerInsetVertical),
            row.bottomAnchor.constraint(
                equalTo: bottomAnchor, constant: -visual.metrics.composerInsetVertical),
            editor.heightAnchor.constraint(greaterThanOrEqualToConstant: 38),
            heightAnchor.constraint(greaterThanOrEqualToConstant: visual.metrics.composerMinHeight),
            heightAnchor.constraint(lessThanOrEqualToConstant: visual.metrics.composerMaxHeight),
        ])
    }

    func focusEditor() {
        guard let editor, let window else { return }
        window.makeFirstResponder(editor)
    }

    func setBusy(_ busy: Bool, process: String) {
        guard let editor else { return }
        editor.isEditable = !busy
        editor.alphaValue = busy ? 0.55 : 1
        editor.setAccessibilityValue(busy ? "Busy: \(process)" : "Available")
    }

    func clearAcceptedDraft() {
        editor?.string = ""
    }

    private func buildBusyState(process: String) {
        SeyalMaterialPresenter.apply(
            .recededUtility,
            to: self,
            visual: visual,
            cornerRadius: visual.metrics.composerCornerRadius
        )

        let dot = NSTextField(labelWithString: "●")
        dot.font = NSFont.systemFont(ofSize: 9, weight: .bold)
        dot.textColor = visual.colors.ns(.warning)

        let status = NSTextField(
            labelWithString: "\(process) is using this shell · open another Pane to run in parallel")
        status.font = visual.typography[.action]
        status.textColor = visual.colors.ns(.textSecondary)
        status.lineBreakMode = .byTruncatingTail

        let row = NSStackView(views: [dot, status])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = visual.metrics.sm
        row.translatesAutoresizingMaskIntoConstraints = false
        addSubview(row)

        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(
                equalTo: leadingAnchor, constant: visual.metrics.composerInsetHorizontal),
            row.trailingAnchor.constraint(
                lessThanOrEqualTo: trailingAnchor,
                constant: -visual.metrics.composerInsetHorizontal),
            row.centerYAnchor.constraint(equalTo: centerYAnchor),
            heightAnchor.constraint(equalToConstant: visual.metrics.composerMinHeight),
        ])
    }

    func textDidBeginEditing(_ notification: Notification) {
        onFocus?()
    }

    func textDidChange(_ notification: Notification) {
        guard let editor else { return }
        onDraftChange?(editor.string)
    }

    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        guard PaneComposerTextView.isReturnSelector(selector),
              let event = NSApp.currentEvent,
              !event.modifierFlags.contains(.shift)
        else { return false }
        let command = textView.string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !command.isEmpty else { return true }
        _ = onSubmit?(command)
        return true
    }
}
