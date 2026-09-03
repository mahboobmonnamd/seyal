import AppKit

@MainActor
final class BlockView: NSView {
    private var presentation: BlockPresentation
    private let bodyView: NSView
    private var visual: SeyalResolvedVisualConfiguration
    private var stateMark: NSTextField?
    private var commandField: NSTextField?
    private var elapsedField: NSTextField?
    private weak var chromeHeader: NSView?
    private weak var seam: SeyalSemanticSeamView?
    private weak var contentStack: NSStackView?

    init(
        presentation: BlockPresentation,
        bodyView: NSView,
        visual: SeyalResolvedVisualConfiguration
    ) {
        self.presentation = presentation
        self.bodyView = bodyView
        self.visual = visual
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        setAccessibilityIdentifier("block.\(presentation.id)")
        buildUI()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("BlockView is programmatic")
    }

    var presentationState: BlockPresentationState { presentation.state }

    func applyVisual(_ visual: SeyalResolvedVisualConfiguration) {
        self.visual = visual
        applyPresentationChrome()
        commandField?.font = visual.typography[.composer]
        commandField?.textColor = visual.colors.ns(.textPrimary)
        elapsedField?.font = visual.typography[.metadata]
        elapsedField?.textColor = visual.colors.ns(.textSecondary)
        seam?.apply(visual: visual, role: seamRole)
    }

    func apply(presentation: BlockPresentation) {
        guard presentation.id == self.presentation.id else { return }
        self.presentation = presentation
        stateMark?.textColor = stateColor(for: presentation.state)
        stateMark?.toolTip = presentation.state.rawValue
        commandField?.stringValue = presentation.command
        elapsedField?.stringValue = presentation.elapsed
        applyPresentationChrome()
        seam?.apply(role: seamRole)
    }

    private var seamRole: SeyalSeamRole {
        if presentation.isSelected { return .focus }
        switch presentation.state {
        case .running: return .running
        case .failed: return .attention
        case .completed: return .rest
        }
    }

    private func buildUI() {
        wantsLayer = true
        layer?.cornerRadius = visual.metrics.blockCornerRadius
        applyPresentationChrome()

        let stateMark = NSTextField(labelWithString: "●")
        stateMark.font = NSFont.systemFont(ofSize: 9, weight: .bold)
        stateMark.textColor = stateColor(for: presentation.state)
        stateMark.toolTip = presentation.state.rawValue
        self.stateMark = stateMark

        let commandField = NSTextField(labelWithString: presentation.command)
        commandField.font = visual.typography[.composer]
        commandField.textColor = visual.colors.ns(.textPrimary)
        commandField.lineBreakMode = .byTruncatingMiddle
        commandField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        self.commandField = commandField

        let commandStack = NSStackView(views: [stateMark, commandField])
        commandStack.orientation = .horizontal
        commandStack.alignment = .centerY
        commandStack.spacing = visual.metrics.controlSpacing - 1
        commandStack.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let actions = makeActions()
        let elapsedField = metadataField(presentation.elapsed, color: visual.colors.ns(.textSecondary))
        self.elapsedField = elapsedField
        var metadataViews: [NSView] = [elapsedField]
        if let timestamp = presentation.timestamp {
            metadataViews.append(metadataField(timestamp, color: visual.colors.ns(.textMuted)))
        }

        let metadataStack = NSStackView(views: metadataViews)
        metadataStack.orientation = .horizontal
        metadataStack.alignment = .centerY
        metadataStack.spacing = visual.metrics.sm

        let rightStack = NSStackView(views: [actions, metadataStack])
        rightStack.orientation = .horizontal
        rightStack.alignment = .centerY
        rightStack.spacing = visual.metrics.md

        let header = NSStackView(views: [commandStack, rightStack])
        header.orientation = .horizontal
        header.alignment = .centerY
        header.distribution = .fill
        header.spacing = visual.metrics.sm
        header.translatesAutoresizingMaskIntoConstraints = false

        let seam = SeyalSemanticSeamView(visual: visual, role: seamRole)
        self.seam = seam

        bodyView.translatesAutoresizingMaskIntoConstraints = false

        let stack = NSStackView(views: [header, bodyView, seam])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = visual.metrics.blockSeamSpacing / 2
        stack.edgeInsets = NSEdgeInsets(
            top: visual.metrics.contentPaddingVertical,
            left: visual.metrics.contentPaddingHorizontal,
            bottom: visual.metrics.xs,
            right: visual.metrics.contentPaddingHorizontal
        )
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        chromeHeader = header
        contentStack = stack

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            header.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            seam.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            seam.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            bodyView.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            bodyView.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
        ])
    }

    func setTUITakeover(_ active: Bool) {
        chromeHeader?.isHidden = active
        seam?.isHidden = active
        contentStack?.spacing = active ? 0 : visual.metrics.blockSeamSpacing / 2
        contentStack?.edgeInsets = active
            ? NSEdgeInsets(top: 0, left: 0, bottom: 0, right: 0)
            : NSEdgeInsets(
                top: visual.metrics.contentPaddingVertical,
                left: visual.metrics.contentPaddingHorizontal,
                bottom: visual.metrics.xs,
                right: visual.metrics.contentPaddingHorizontal
            )
        layer?.backgroundColor = active
            ? NSColor.clear.cgColor
            : (presentation.isSelected ? visual.colors.cg(.selection) : NSColor.clear.cgColor)
        layer?.borderWidth = 0
    }

    private func applyPresentationChrome() {
        layer?.borderWidth = 0
        layer?.cornerRadius = visual.metrics.blockCornerRadius
        layer?.backgroundColor = presentation.isSelected
            ? visual.colors.cg(.selection)
            : NSColor.clear.cgColor
    }

    private func makeActions() -> NSView {
        let actionViews = presentation.actions.map { action -> NSTextField in
            let field = NSTextField(labelWithString: action)
            field.font = visual.typography[.metadata]
            field.textColor = visual.colors.ns(.textMuted)
            return field
        }

        let stack = NSStackView(views: actionViews)
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = visual.metrics.sm + 1
        return stack
    }

    private func metadataField(_ value: String, color: NSColor) -> NSTextField {
        let field = NSTextField(labelWithString: value)
        field.font = visual.typography[.metadata]
        field.textColor = color
        field.alignment = .right
        return field
    }

    private func stateColor(for state: BlockPresentationState) -> NSColor {
        switch state {
        case .running:
            visual.colors.ns(.warning)
        case .completed:
            visual.colors.ns(.success)
        case .failed:
            visual.colors.ns(.danger)
        }
    }
}
