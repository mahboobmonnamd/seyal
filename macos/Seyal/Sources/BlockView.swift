import AppKit

@MainActor
final class BlockView: NSView {
    private let presentation: BlockPresentation
    private let bodyView: NSView

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
        layer?.borderWidth = presentation.isSelected ? 1.5 : 1
        layer?.borderColor = (presentation.isSelected
            ? SeyalDesignTokens.Palette.focus
            : SeyalDesignTokens.Palette.separator).cgColor
        layer?.backgroundColor = SeyalDesignTokens.Palette.paneBackground.cgColor

        let commandField = NSTextField(labelWithString: presentation.command)
        commandField.font = SeyalDesignTokens.Typography.command
        commandField.textColor = SeyalDesignTokens.Palette.textPrimary
        commandField.lineBreakMode = .byTruncatingMiddle
        commandField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let stateField = metadataField(presentation.state.rawValue, color: stateColor(for: presentation.state))
        let elapsedField = metadataField(presentation.elapsed, color: SeyalDesignTokens.Palette.textSecondary)

        var metadataViews: [NSView] = [stateField, elapsedField]
        if let timestamp = presentation.timestamp {
            metadataViews.append(metadataField(timestamp, color: SeyalDesignTokens.Palette.textTertiary))
        }

        let metadataStack = NSStackView(views: metadataViews)
        metadataStack.orientation = .horizontal
        metadataStack.alignment = .centerY
        metadataStack.spacing = 8

        let header = NSStackView(views: [commandField, metadataStack])
        header.orientation = .horizontal
        header.alignment = .centerY
        header.spacing = SeyalDesignTokens.Layout.compactSpacing
        header.translatesAutoresizingMaskIntoConstraints = false

        let divider = NSBox()
        divider.boxType = .separator

        bodyView.translatesAutoresizingMaskIntoConstraints = false

        let stack = NSStackView(views: [header, divider, bodyView])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = SeyalDesignTokens.Layout.compactSpacing
        stack.edgeInsets = NSEdgeInsets(
            top: 10,
            left: SeyalDesignTokens.Layout.standardSpacing,
            bottom: 12,
            right: SeyalDesignTokens.Layout.standardSpacing
        )
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

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

#if DEBUG
@MainActor
final class PreviewTerminalFixtureView: NSView {
    init(lines: [String]) {
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false

        let lineViews = lines.map { line -> NSTextField in
            let field = NSTextField(labelWithString: line)
            field.font = SeyalDesignTokens.Typography.terminal
            field.textColor = SeyalDesignTokens.Palette.textPrimary
            field.lineBreakMode = .byClipping
            field.maximumNumberOfLines = 1
            return field
        }

        let stack = NSStackView(views: lineViews)
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 3
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("PreviewTerminalFixtureView is programmatic")
    }
}
#endif
