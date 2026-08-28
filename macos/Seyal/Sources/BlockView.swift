import AppKit

@MainActor
final class BlockView: NSView {
    private let presentation: BlockPresentation
    private let bodyView: NSView
    private weak var chromeHeader: NSView?
    private weak var chromeDivider: NSView?
    private weak var contentStack: NSStackView?

    init(presentation: BlockPresentation, bodyView: NSView) {
        self.presentation = presentation
        self.bodyView = bodyView
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        buildUI()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("BlockView is programmatic")
    }

    private func buildUI() {
        wantsLayer = true
        layer?.cornerRadius = SeyalDesignTokens.Layout.blockCornerRadius
        layer?.borderWidth = presentation.isSelected ? 0.75 : 0.5
        layer?.borderColor = (presentation.isSelected
            ? SeyalDesignTokens.Palette.focus
            : SeyalDesignTokens.Palette.separator).cgColor
        layer?.backgroundColor = (presentation.isSelected
            ? SeyalDesignTokens.Palette.blockSelectedBackground
            : SeyalDesignTokens.Palette.blockBackground).cgColor

        let stateMark = NSTextField(labelWithString: "●")
        stateMark.font = NSFont.systemFont(ofSize: 9, weight: .bold)
        stateMark.textColor = stateColor(for: presentation.state)
        stateMark.toolTip = presentation.state.rawValue

        let commandField = NSTextField(labelWithString: presentation.command)
        commandField.font = SeyalDesignTokens.Typography.command
        commandField.textColor = SeyalDesignTokens.Palette.textPrimary
        commandField.lineBreakMode = .byTruncatingMiddle
        commandField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let commandStack = NSStackView(views: [stateMark, commandField])
        commandStack.orientation = .horizontal
        commandStack.alignment = .centerY
        commandStack.spacing = 7
        commandStack.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let actions = makeActions()
        let elapsedField = metadataField(presentation.elapsed, color: SeyalDesignTokens.Palette.textSecondary)
        var metadataViews: [NSView] = [elapsedField]
        if let timestamp = presentation.timestamp {
            metadataViews.append(metadataField(timestamp, color: SeyalDesignTokens.Palette.textTertiary))
        }

        let metadataStack = NSStackView(views: metadataViews)
        metadataStack.orientation = .horizontal
        metadataStack.alignment = .centerY
        metadataStack.spacing = 8

        let rightStack = NSStackView(views: [actions, metadataStack])
        rightStack.orientation = .horizontal
        rightStack.alignment = .centerY
        rightStack.spacing = 12

        let header = NSStackView(views: [commandStack, rightStack])
        header.orientation = .horizontal
        header.alignment = .centerY
        header.distribution = .fill
        header.spacing = SeyalDesignTokens.Layout.standardSpacing
        header.translatesAutoresizingMaskIntoConstraints = false

        let divider = NSView()
        divider.translatesAutoresizingMaskIntoConstraints = false
        divider.wantsLayer = true
        divider.layer?.backgroundColor = SeyalDesignTokens.Palette.subtleSeparator.cgColor
        divider.heightAnchor.constraint(equalToConstant: 1).isActive = true

        bodyView.translatesAutoresizingMaskIntoConstraints = false

        let stack = NSStackView(views: [header, divider, bodyView])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = SeyalDesignTokens.Layout.compactSpacing
        stack.edgeInsets = NSEdgeInsets(top: 10, left: 12, bottom: 12, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        chromeHeader = header
        chromeDivider = divider
        contentStack = stack

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            header.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            divider.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            divider.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            bodyView.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            bodyView.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
        ])
    }

    /// TUI takeover changes presentation chrome only. The body remains the
    /// same bridge-backed surface and keeps native input/focus ownership.
    func setTUITakeover(_ active: Bool) {
        chromeHeader?.isHidden = active
        chromeDivider?.isHidden = active
        contentStack?.spacing = active ? 0 : SeyalDesignTokens.Layout.compactSpacing
        contentStack?.edgeInsets = active
            ? NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
            : NSEdgeInsets(top: 10, left: 12, bottom: 12, right: 12)
        layer?.backgroundColor = active
            ? NSColor.clear.cgColor
            : (presentation.isSelected
                ? SeyalDesignTokens.Palette.blockSelectedBackground
                : SeyalDesignTokens.Palette.blockBackground).cgColor
        layer?.borderWidth = active ? 0 : (presentation.isSelected ? 0.75 : 0.5)
    }

    /// Action labels remain a presentation seam. Runtime-dependent actions are
    /// intentionally empty until their authoritative backing exists.
    private func makeActions() -> NSView {
        let actionViews = presentation.actions.map { action -> NSTextField in
            let field = NSTextField(labelWithString: action)
            field.font = SeyalDesignTokens.Typography.metadataEmphasized
            field.textColor = SeyalDesignTokens.Palette.textTertiary
            return field
        }

        let stack = NSStackView(views: actionViews)
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = 9
        return stack
    }

    private func metadataField(_ value: String, color: NSColor) -> NSTextField {
        let field = NSTextField(labelWithString: value)
        field.font = SeyalDesignTokens.Typography.metadata
        field.textColor = color
        field.alignment = .right
        return field
    }

    private func stateColor(for state: BlockPresentationState) -> NSColor {
        switch state {
        case .running:
            SeyalDesignTokens.Palette.warning
        case .completed:
            SeyalDesignTokens.Palette.success
        case .failed:
            SeyalDesignTokens.Palette.failure
        }
    }
}
