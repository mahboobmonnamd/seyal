import AppKit

@MainActor
final class SeyalShellView: NSView {
    #if DEBUG
    struct LayoutContract {
        let topChrome: NSRect
        let leftContext: NSRect
        let pane: NSRect
        let inspector: NSRect
        let composer: NSRect
    }
    #endif

    private let snapshot: SeyalShellSnapshot
    private let blocks: [BlockPresentation]
    private let blockBodies: [NSView]
    private var attentionPopover: NSPopover?

    private weak var topChromeView: NSView?
    private weak var leftContextView: NSView?
    private weak var paneView: NSView?
    private weak var inspectorView: NSView?
    private weak var composerView: NSView?

    init(
        frame frameRect: NSRect,
        snapshot: SeyalShellSnapshot,
        blocks: [BlockPresentation],
        blockBodies: [NSView]
    ) {
        precondition(blocks.count == blockBodies.count, "Each Block presentation requires exactly one body view")
        self.snapshot = snapshot
        self.blocks = blocks
        self.blockBodies = blockBodies
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = SeyalDesignTokens.Palette.windowBackground.cgColor
        buildUI()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("SeyalShellView is programmatic")
    }

    private func buildUI() {
        let topChrome = makeTopChrome()
        let leftContext = makeLeftContextPanel()
        let pane = makePane()
        let inspector = makeInspector()
        topChromeView = topChrome
        leftContextView = leftContext
        paneView = pane
        inspectorView = inspector

        let body = NSStackView(views: [leftContext, pane, inspector])
        body.orientation = .horizontal
        body.alignment = .top
        body.spacing = 1
        body.translatesAutoresizingMaskIntoConstraints = false
        body.wantsLayer = true
        body.layer?.backgroundColor = SeyalDesignTokens.Palette.separator.cgColor

        let root = NSStackView(views: [topChrome, body])
        root.orientation = .vertical
        root.alignment = .leading
        root.spacing = 1
        root.translatesAutoresizingMaskIntoConstraints = false
        addSubview(root)

        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: leadingAnchor),
            root.trailingAnchor.constraint(equalTo: trailingAnchor),
            root.topAnchor.constraint(equalTo: topAnchor),
            root.bottomAnchor.constraint(equalTo: bottomAnchor),

            topChrome.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            topChrome.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            topChrome.heightAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.topChromeHeight),

            body.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            body.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            body.bottomAnchor.constraint(equalTo: root.bottomAnchor),

            leftContext.widthAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.leftContextWidth),
            inspector.widthAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.inspectorWidth),
            pane.widthAnchor.constraint(greaterThanOrEqualToConstant: 520),
            leftContext.heightAnchor.constraint(equalTo: body.heightAnchor),
            pane.heightAnchor.constraint(equalTo: body.heightAnchor),
            inspector.heightAnchor.constraint(equalTo: body.heightAnchor),
        ])
    }

    private func makeTopChrome() -> NSView {
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        container.layer?.backgroundColor = SeyalDesignTokens.Palette.chromeBackground.cgColor

        let workspace = snapshot.workspaces.first { $0.id == snapshot.activeWorkspaceID }
        let workspaceField = NSTextField(labelWithString: workspace?.name ?? "Workspace")
        workspaceField.font = SeyalDesignTokens.Typography.chromeEmphasized
        workspaceField.textColor = SeyalDesignTokens.Palette.textPrimary
        workspaceField.lineBreakMode = .byTruncatingTail
        workspaceField.translatesAutoresizingMaskIntoConstraints = false

        let slash = NSTextField(labelWithString: "/")
        slash.font = SeyalDesignTokens.Typography.chrome
        slash.textColor = SeyalDesignTokens.Palette.textTertiary
        slash.translatesAutoresizingMaskIntoConstraints = false

        let tabs = makeTabStrip()
        tabs.translatesAutoresizingMaskIntoConstraints = false

        let attentionButton = NSButton(title: "", target: self, action: #selector(showAttention(_:)))
        attentionButton.bezelStyle = .inline
        attentionButton.isBordered = false
        attentionButton.image = NSImage(systemSymbolName: "bell", accessibilityDescription: "Attention")
        attentionButton.imagePosition = .imageLeading
        if !snapshot.attentionItems.isEmpty {
            attentionButton.title = " \(snapshot.attentionItems.count)"
        }
        attentionButton.contentTintColor = SeyalDesignTokens.Palette.textSecondary
        attentionButton.font = SeyalDesignTokens.Typography.chrome
        attentionButton.toolTip = "Attention"
        attentionButton.translatesAutoresizingMaskIntoConstraints = false

        container.addSubview(workspaceField)
        container.addSubview(slash)
        container.addSubview(tabs)
        container.addSubview(attentionButton)

        NSLayoutConstraint.activate([
            workspaceField.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 14),
            workspaceField.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            workspaceField.widthAnchor.constraint(lessThanOrEqualToConstant: 150),

            slash.leadingAnchor.constraint(equalTo: workspaceField.trailingAnchor, constant: 7),
            slash.centerYAnchor.constraint(equalTo: container.centerYAnchor),

            tabs.leadingAnchor.constraint(equalTo: slash.trailingAnchor, constant: 8),
            tabs.trailingAnchor.constraint(equalTo: attentionButton.leadingAnchor, constant: -8),
            tabs.topAnchor.constraint(equalTo: container.topAnchor, constant: 4),
            tabs.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -4),

            attentionButton.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -12),
            attentionButton.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            attentionButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 34),
        ])

        return container
    }

    private func makeTabStrip() -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasHorizontalScroller = false
        scroll.hasVerticalScroller = false
        scroll.horizontalScrollElasticity = .allowed
        scroll.verticalScrollElasticity = .none

        let tabStack = NSStackView()
        tabStack.orientation = .horizontal
        tabStack.alignment = .centerY
        tabStack.spacing = 3
        tabStack.translatesAutoresizingMaskIntoConstraints = false

        snapshot.tabs.forEach { tab in
            tabStack.addArrangedSubview(makeTabChip(tab))
        }

        let document = NSView()
        document.translatesAutoresizingMaskIntoConstraints = false
        document.addSubview(tabStack)
        scroll.documentView = document

        NSLayoutConstraint.activate([
            tabStack.leadingAnchor.constraint(equalTo: document.leadingAnchor),
            tabStack.trailingAnchor.constraint(equalTo: document.trailingAnchor),
            tabStack.topAnchor.constraint(equalTo: document.topAnchor),
            tabStack.bottomAnchor.constraint(equalTo: document.bottomAnchor),
            document.heightAnchor.constraint(equalTo: scroll.contentView.heightAnchor),
            document.widthAnchor.constraint(greaterThanOrEqualTo: scroll.contentView.widthAnchor),
        ])

        return scroll
    }

    private func makeTabChip(_ tab: SeyalShellSnapshot.Tab) -> NSView {
        let dot = NSTextField(labelWithString: "●")
        dot.font = NSFont.systemFont(ofSize: 7, weight: .bold)
        dot.textColor = tab.attention
            ? SeyalDesignTokens.Palette.warning
            : (tab.id == snapshot.activeTabID
                ? SeyalDesignTokens.Palette.success
                : SeyalDesignTokens.Palette.textTertiary)

        let field = NSTextField(labelWithString: tab.title)
        field.font = tab.id == snapshot.activeTabID
            ? SeyalDesignTokens.Typography.chromeEmphasized
            : SeyalDesignTokens.Typography.chrome
        field.textColor = tab.id == snapshot.activeTabID
            ? SeyalDesignTokens.Palette.textPrimary
            : SeyalDesignTokens.Palette.textSecondary
        field.lineBreakMode = .byTruncatingTail

        let labelStack = NSStackView(views: [dot, field])
        labelStack.orientation = .horizontal
        labelStack.alignment = .centerY
        labelStack.spacing = 6
        labelStack.translatesAutoresizingMaskIntoConstraints = false

        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        container.layer?.cornerRadius = 7
        container.layer?.backgroundColor = tab.id == snapshot.activeTabID
            ? SeyalDesignTokens.Palette.elevatedBackground.cgColor
            : NSColor.clear.cgColor
        container.addSubview(labelStack)

        if tab.id == snapshot.activeTabID {
            let accent = NSView()
            accent.translatesAutoresizingMaskIntoConstraints = false
            accent.wantsLayer = true
            accent.layer?.backgroundColor = SeyalDesignTokens.Palette.focus.cgColor
            container.addSubview(accent)
            NSLayoutConstraint.activate([
                accent.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 10),
                accent.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -10),
                accent.bottomAnchor.constraint(equalTo: container.bottomAnchor),
                accent.heightAnchor.constraint(equalToConstant: 2),
            ])
        }

        NSLayoutConstraint.activate([
            labelStack.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 10),
            labelStack.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -10),
            labelStack.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            container.widthAnchor.constraint(greaterThanOrEqualToConstant: SeyalDesignTokens.Layout.tabMinWidth),
            container.widthAnchor.constraint(lessThanOrEqualToConstant: SeyalDesignTokens.Layout.tabMaxWidth),
        ])
        return container
    }

    private func makeLeftContextPanel() -> NSView {
        let panel = NSView()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.wantsLayer = true
        panel.layer?.backgroundColor = SeyalDesignTokens.Palette.panelBackground.cgColor

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 6
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false

        appendSectionTitle("Workspaces", to: stack)
        snapshot.workspaces.forEach { workspace in
            let count = workspace.tabCount == 1 ? "1 tab" : "\(workspace.tabCount) tabs"
            stack.addArrangedSubview(makeContextRow(
                primary: workspace.name,
                secondary: workspace.detail,
                trailing: count,
                emphasized: workspace.id == snapshot.activeWorkspaceID,
                attention: workspace.attention,
                statusColor: workspace.id == snapshot.activeWorkspaceID
                    ? SeyalDesignTokens.Palette.success
                    : nil
            ))
        }

        stack.addArrangedSubview(makeSpacer(height: 8))
        appendSectionTitle("Agents", to: stack)
        snapshot.agents.forEach { agent in
            stack.addArrangedSubview(makeContextRow(
                primary: agent.name,
                secondary: nil,
                trailing: agent.state.rawValue,
                emphasized: false,
                attention: agent.state == .attention,
                statusColor: agentColor(agent.state)
            ))
        }

        stack.addArrangedSubview(makeSpacer(height: 8))
        appendSectionTitle("Tabs", to: stack)
        snapshot.tabs.forEach { tab in
            let paneDetail = tab.paneCount == 1 ? "1 pane" : "\(tab.paneCount) panes"
            stack.addArrangedSubview(makeContextRow(
                primary: tab.title,
                secondary: nil,
                trailing: paneDetail,
                emphasized: tab.id == snapshot.activeTabID,
                attention: tab.attention,
                statusColor: tab.attention ? SeyalDesignTokens.Palette.warning : nil
            ))
        }

        panel.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            stack.topAnchor.constraint(equalTo: panel.topAnchor),
        ])
        return panel
    }

    private func makePane() -> NSView {
        let pane = NSView()
        pane.translatesAutoresizingMaskIntoConstraints = false
        pane.wantsLayer = true
        pane.layer?.backgroundColor = SeyalDesignTokens.Palette.paneBackground.cgColor

        let title = NSTextField(labelWithString: "Core Terminal")
        title.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        title.textColor = SeyalDesignTokens.Palette.textPrimary

        let context = NSTextField(labelWithString: "main  ·  ~/Projects/seyal  ·  zsh")
        context.font = SeyalDesignTokens.Typography.metadata
        context.textColor = SeyalDesignTokens.Palette.textSecondary

        let titleStack = NSStackView(views: [title, context])
        titleStack.orientation = .vertical
        titleStack.alignment = .leading
        titleStack.spacing = 2

        let paneCount = NSTextField(labelWithString: "1 pane")
        paneCount.font = SeyalDesignTokens.Typography.metadata
        paneCount.textColor = SeyalDesignTokens.Palette.textTertiary

        let paneHeader = NSStackView(views: [titleStack, paneCount])
        paneHeader.orientation = .horizontal
        paneHeader.alignment = .centerY
        paneHeader.spacing = 12
        paneHeader.edgeInsets = NSEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)
        paneHeader.translatesAutoresizingMaskIntoConstraints = false

        let transcript = makeTranscript()
        transcript.translatesAutoresizingMaskIntoConstraints = false
        transcript.setContentHuggingPriority(.defaultLow, for: .vertical)
        transcript.setContentCompressionResistancePriority(.defaultLow, for: .vertical)

        let composer = PaneComposerShellView(mode: .available, draft: "")
        composerView = composer

        let stack = NSStackView(views: [paneHeader, transcript, composer])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 0, left: 8, bottom: 8, right: 8)
        stack.translatesAutoresizingMaskIntoConstraints = false
        pane.addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: pane.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: pane.trailingAnchor),
            stack.topAnchor.constraint(equalTo: pane.topAnchor),
            stack.bottomAnchor.constraint(equalTo: pane.bottomAnchor),
            paneHeader.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            paneHeader.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            transcript.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            transcript.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            transcript.heightAnchor.constraint(greaterThanOrEqualToConstant: 320),
            composer.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            composer.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
        ])

        return pane
    }

    /// The Pane owns normal Block/transcript scrolling. BlockView intentionally
    /// contains no NSScrollView, preserving the frozen single-scroll-owner rule.
    private func makeTranscript() -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder

        let document = NSView()
        document.translatesAutoresizingMaskIntoConstraints = false

        let blockStack = NSStackView()
        blockStack.orientation = .vertical
        blockStack.alignment = .leading
        blockStack.spacing = SeyalDesignTokens.Layout.standardSpacing
        blockStack.edgeInsets = NSEdgeInsets(top: 8, left: 10, bottom: 16, right: 10)
        blockStack.translatesAutoresizingMaskIntoConstraints = false

        for (presentation, body) in zip(blocks, blockBodies) {
            let block = BlockView(presentation: presentation, bodyView: body)
            blockStack.addArrangedSubview(block)
            block.widthAnchor.constraint(equalTo: blockStack.widthAnchor, constant: -20).isActive = true
        }

        document.addSubview(blockStack)
        scroll.documentView = document

        NSLayoutConstraint.activate([
            blockStack.leadingAnchor.constraint(equalTo: document.leadingAnchor),
            blockStack.trailingAnchor.constraint(equalTo: document.trailingAnchor),
            blockStack.topAnchor.constraint(equalTo: document.topAnchor),
            blockStack.bottomAnchor.constraint(equalTo: document.bottomAnchor),
            document.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor),
        ])

        return scroll
    }

    private func makeInspector() -> NSView {
        let panel = NSView()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.wantsLayer = true
        panel.layer?.backgroundColor = SeyalDesignTokens.Palette.panelBackground.cgColor

        let title = NSTextField(labelWithString: "Inspector")
        title.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        title.textColor = SeyalDesignTokens.Palette.textPrimary

        let mode = NSTextField(labelWithString: "CONTEXT")
        mode.font = SeyalDesignTokens.Typography.section
        mode.textColor = SeyalDesignTokens.Palette.focus

        let stack = NSStackView(views: [title, mode])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false

        var currentSection: String?
        for row in snapshot.inspectorRows {
            if currentSection != row.section {
                if currentSection != nil {
                    stack.addArrangedSubview(makeSpacer(height: 5))
                }
                appendSectionTitle(row.section, to: stack)
                currentSection = row.section
            }
            stack.addArrangedSubview(makeInspectorRow(row))
        }

        panel.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            stack.topAnchor.constraint(equalTo: panel.topAnchor),
        ])
        return panel
    }

    private func makeInspectorRow(_ row: SeyalShellSnapshot.InspectorRow) -> NSView {
        let label = NSTextField(labelWithString: row.label)
        label.font = SeyalDesignTokens.Typography.metadata
        label.textColor = SeyalDesignTokens.Palette.textTertiary
        label.setContentCompressionResistancePriority(.required, for: .horizontal)

        let value = NSTextField(labelWithString: row.value)
        value.font = SeyalDesignTokens.Typography.body
        value.textColor = SeyalDesignTokens.Palette.textPrimary
        value.lineBreakMode = .byTruncatingMiddle
        value.alignment = .right
        value.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let rowStack = NSStackView(views: [label, value])
        rowStack.orientation = .horizontal
        rowStack.alignment = .centerY
        rowStack.distribution = .fill
        rowStack.spacing = 8
        rowStack.translatesAutoresizingMaskIntoConstraints = false
        rowStack.widthAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.inspectorWidth - 24).isActive = true
        return rowStack
    }

    private func appendSectionTitle(_ title: String, to stack: NSStackView) {
        let field = NSTextField(labelWithString: title.uppercased())
        field.font = SeyalDesignTokens.Typography.section
        field.textColor = SeyalDesignTokens.Palette.textTertiary
        stack.addArrangedSubview(field)
    }

    private func makeContextRow(
        primary: String,
        secondary: String?,
        trailing: String?,
        emphasized: Bool,
        attention: Bool,
        statusColor: NSColor?
    ) -> NSView {
        let dot = NSTextField(labelWithString: "●")
        dot.font = NSFont.systemFont(ofSize: 7, weight: .bold)
        dot.textColor = statusColor ?? (attention
            ? SeyalDesignTokens.Palette.warning
            : SeyalDesignTokens.Palette.textTertiary)

        let primaryField = NSTextField(labelWithString: primary)
        primaryField.font = emphasized
            ? SeyalDesignTokens.Typography.bodyEmphasized
            : SeyalDesignTokens.Typography.body
        primaryField.textColor = emphasized
            ? SeyalDesignTokens.Palette.textPrimary
            : SeyalDesignTokens.Palette.textSecondary
        primaryField.lineBreakMode = .byTruncatingTail
        primaryField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        let topViews: [NSView]
        if let trailing {
            let trailingField = NSTextField(labelWithString: trailing)
            trailingField.font = SeyalDesignTokens.Typography.metadata
            trailingField.textColor = attention
                ? SeyalDesignTokens.Palette.warning
                : SeyalDesignTokens.Palette.textTertiary
            trailingField.alignment = .right
            topViews = [dot, primaryField, trailingField]
        } else {
            topViews = [dot, primaryField]
        }

        let top = NSStackView(views: topViews)
        top.orientation = .horizontal
        top.alignment = .centerY
        top.distribution = .fill
        top.spacing = 6

        var views: [NSView] = [top]
        if let secondary {
            let secondaryField = NSTextField(labelWithString: secondary)
            secondaryField.font = SeyalDesignTokens.Typography.metadata
            secondaryField.textColor = SeyalDesignTokens.Palette.textTertiary
            secondaryField.lineBreakMode = .byTruncatingMiddle
            secondaryField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            views.append(secondaryField)
        }

        let row = NSStackView(views: views)
        row.orientation = .vertical
        row.alignment = .leading
        row.spacing = 2
        row.edgeInsets = NSEdgeInsets(top: 5, left: 7, bottom: 5, right: 7)
        row.translatesAutoresizingMaskIntoConstraints = false
        row.widthAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.leftContextWidth - 24).isActive = true
        row.wantsLayer = true
        row.layer?.cornerRadius = 7
        row.layer?.backgroundColor = emphasized
            ? SeyalDesignTokens.Palette.focusSoft.cgColor
            : NSColor.clear.cgColor
        top.widthAnchor.constraint(equalTo: row.widthAnchor, constant: -14).isActive = true
        return row
    }

    private func agentColor(_ state: SeyalShellSnapshot.Agent.State) -> NSColor {
        switch state {
        case .running:
            SeyalDesignTokens.Palette.success
        case .waiting:
            SeyalDesignTokens.Palette.info
        case .attention:
            SeyalDesignTokens.Palette.warning
        case .idle:
            SeyalDesignTokens.Palette.textTertiary
        }
    }

    private func makeSpacer(height: CGFloat) -> NSView {
        let spacer = NSView()
        spacer.translatesAutoresizingMaskIntoConstraints = false
        spacer.heightAnchor.constraint(equalToConstant: height).isActive = true
        return spacer
    }

    @objc
    private func showAttention(_ sender: NSButton) {
        guard !snapshot.attentionItems.isEmpty else { return }

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.wantsLayer = true
        stack.layer?.backgroundColor = SeyalDesignTokens.Palette.elevatedBackground.cgColor

        appendSectionTitle("Attention", to: stack)
        snapshot.attentionItems.forEach { item in
            let title = NSTextField(labelWithString: item.title)
            title.font = SeyalDesignTokens.Typography.bodyEmphasized
            title.textColor = SeyalDesignTokens.Palette.textPrimary

            let detail = NSTextField(wrappingLabelWithString: item.detail)
            detail.font = SeyalDesignTokens.Typography.body
            detail.textColor = SeyalDesignTokens.Palette.textSecondary
            detail.maximumNumberOfLines = 2

            let itemStack = NSStackView(views: [title, detail])
            itemStack.orientation = .vertical
            itemStack.alignment = .leading
            itemStack.spacing = 2
            stack.addArrangedSubview(itemStack)
            itemStack.widthAnchor.constraint(equalToConstant: 300).isActive = true
        }

        let controller = NSViewController()
        controller.view = stack
        controller.preferredContentSize = NSSize(
            width: 324,
            height: max(CGFloat(120), CGFloat(snapshot.attentionItems.count) * 66 + 44)
        )

        let popover = NSPopover()
        popover.behavior = .transient
        popover.appearance = NSAppearance(named: .darkAqua)
        popover.contentViewController = controller
        popover.show(relativeTo: sender.bounds, of: sender, preferredEdge: .maxY)
        attentionPopover = popover
    }

    #if DEBUG
    func debugLayoutContract() -> LayoutContract? {
        layoutSubtreeIfNeeded()
        guard let topChromeView,
              let leftContextView,
              let paneView,
              let inspectorView,
              let composerView else {
            return nil
        }
        return LayoutContract(
            topChrome: topChromeView.convert(topChromeView.bounds, to: self),
            leftContext: leftContextView.convert(leftContextView.bounds, to: self),
            pane: paneView.convert(paneView.bounds, to: self),
            inspector: inspectorView.convert(inspectorView.bounds, to: self),
            composer: composerView.convert(composerView.bounds, to: self)
        )
    }
    #endif

    static func smokeTest() -> Bool {
        #if DEBUG
        let shell = SeyalShellPreviewFactory.make(frame: NSRect(x: 0, y: 0, width: 1280, height: 800))
        shell.layoutSubtreeIfNeeded()
        guard let contract = shell.debugLayoutContract() else { return false }
        return shell.subviews.count == 1
            && abs(contract.topChrome.height - SeyalDesignTokens.Layout.topChromeHeight) < 1
            && abs(contract.leftContext.width - SeyalDesignTokens.Layout.leftContextWidth) < 1
            && abs(contract.inspector.width - SeyalDesignTokens.Layout.inspectorWidth) < 1
            && contract.pane.width > 600
        #else
        return true
        #endif
    }
}
