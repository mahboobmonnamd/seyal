import AppKit

@MainActor
final class SeyalShellView: NSView {
    private let snapshot: SeyalShellSnapshot
    private let blocks: [BlockPresentation]
    private let blockBodies: [NSView]
    private var attentionPopover: NSPopover?

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
        translatesAutoresizingMaskIntoConstraints = false
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

        let body = NSStackView(views: [leftContext, pane, inspector])
        body.orientation = .horizontal
        body.alignment = .top
        body.spacing = 1
        body.translatesAutoresizingMaskIntoConstraints = false

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
            pane.widthAnchor.constraint(greaterThanOrEqualToConstant: 420),
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
        workspaceField.font = SeyalDesignTokens.Typography.chrome
        workspaceField.textColor = SeyalDesignTokens.Palette.textPrimary
        workspaceField.translatesAutoresizingMaskIntoConstraints = false

        let tabs = makeTabStrip()
        tabs.translatesAutoresizingMaskIntoConstraints = false

        let attentionButton = NSButton(title: "", target: self, action: #selector(showAttention(_:)))
        attentionButton.bezelStyle = .inline
        attentionButton.image = NSImage(systemSymbolName: "bell", accessibilityDescription: "Attention")
        attentionButton.imagePosition = .imageLeading
        if !snapshot.attentionItems.isEmpty {
            attentionButton.title = "\(snapshot.attentionItems.count)"
        }
        attentionButton.font = SeyalDesignTokens.Typography.chrome
        attentionButton.toolTip = "Attention"
        attentionButton.translatesAutoresizingMaskIntoConstraints = false

        container.addSubview(workspaceField)
        container.addSubview(tabs)
        container.addSubview(attentionButton)

        NSLayoutConstraint.activate([
            workspaceField.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 14),
            workspaceField.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            workspaceField.widthAnchor.constraint(lessThanOrEqualToConstant: 160),

            tabs.leadingAnchor.constraint(equalTo: workspaceField.trailingAnchor, constant: 14),
            tabs.trailingAnchor.constraint(equalTo: attentionButton.leadingAnchor, constant: -10),
            tabs.topAnchor.constraint(equalTo: container.topAnchor, constant: 5),
            tabs.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -5),

            attentionButton.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -12),
            attentionButton.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            attentionButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 30),
        ])

        return container
    }

    private func makeTabStrip() -> NSView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasHorizontalScroller = false
        scroll.hasVerticalScroller = false
        scroll.horizontalScrollElasticity = .allowed
        scroll.verticalScrollElasticity = .none

        let stack = NSStackView()
        stack.orientation = .horizontal
        stack.alignment = .centerY
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false

        for tab in snapshot.tabs {
            let title = tab.attention ? "\(tab.title)  •" : tab.title
            let button = NSButton(title: title, target: nil, action: nil)
            button.bezelStyle = tab.id == snapshot.activeTabID ? .recessed : .inline
            button.font = SeyalDesignTokens.Typography.chrome
            button.isEnabled = false
            button.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            button.widthAnchor.constraint(greaterThanOrEqualToConstant: SeyalDesignTokens.Layout.tabMinWidth).isActive = true
            button.widthAnchor.constraint(lessThanOrEqualToConstant: SeyalDesignTokens.Layout.tabMaxWidth).isActive = true
            stack.addArrangedSubview(button)
        }

        let document = NSView()
        document.translatesAutoresizingMaskIntoConstraints = false
        document.addSubview(stack)
        scroll.documentView = document

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: document.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: document.trailingAnchor),
            stack.topAnchor.constraint(equalTo: document.topAnchor),
            stack.bottomAnchor.constraint(equalTo: document.bottomAnchor),
            document.heightAnchor.constraint(equalTo: scroll.contentView.heightAnchor),
            document.widthAnchor.constraint(greaterThanOrEqualTo: scroll.contentView.widthAnchor),
        ])

        return scroll
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
            stack.addArrangedSubview(makeContextRow(
                primary: workspace.name,
                secondary: workspace.detail,
                emphasized: workspace.id == snapshot.activeWorkspaceID,
                attention: workspace.attention
            ))
        }

        stack.addArrangedSubview(makeSpacer(height: 8))
        appendSectionTitle("Agents", to: stack)
        snapshot.agents.forEach { agent in
            stack.addArrangedSubview(makeContextRow(
                primary: agent.name,
                secondary: agent.state.rawValue,
                emphasized: false,
                attention: agent.state == .attention
            ))
        }

        stack.addArrangedSubview(makeSpacer(height: 8))
        appendSectionTitle("Tabs", to: stack)
        snapshot.tabs.forEach { tab in
            let paneDetail = tab.paneCount == 1 ? "1 pane" : "\(tab.paneCount) panes"
            stack.addArrangedSubview(makeContextRow(
                primary: tab.title,
                secondary: paneDetail,
                emphasized: tab.id == snapshot.activeTabID,
                attention: tab.attention
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

        let title = NSTextField(labelWithString: "Terminal")
        title.font = SeyalDesignTokens.Typography.bodyEmphasized
        title.textColor = SeyalDesignTokens.Palette.textPrimary

        let subtitle = NSTextField(labelWithString: "Focused Pane · transcript scroll owner")
        subtitle.font = SeyalDesignTokens.Typography.metadata
        subtitle.textColor = SeyalDesignTokens.Palette.textSecondary

        let paneHeader = NSStackView(views: [title, subtitle])
        paneHeader.orientation = .vertical
        paneHeader.alignment = .leading
        paneHeader.spacing = 2
        paneHeader.edgeInsets = NSEdgeInsets(top: 9, left: 12, bottom: 7, right: 12)
        paneHeader.translatesAutoresizingMaskIntoConstraints = false

        let transcript = makeTranscript()
        transcript.translatesAutoresizingMaskIntoConstraints = false

        let composer = PaneComposerShellView(
            mode: .available,
            draft: "git diff --stat && \\\nmake test"
        )

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
            transcript.heightAnchor.constraint(greaterThanOrEqualToConstant: 280),
            composer.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            composer.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
        ])

        return pane
    }

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
        blockStack.edgeInsets = NSEdgeInsets(top: 10, left: 10, bottom: 16, right: 10)
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

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false

        appendSectionTitle("Context", to: stack)
        snapshot.inspectorRows.forEach { row in
            let label = NSTextField(labelWithString: row.label)
            label.font = SeyalDesignTokens.Typography.metadata
            label.textColor = SeyalDesignTokens.Palette.textTertiary

            let value = NSTextField(wrappingLabelWithString: row.value)
            value.font = SeyalDesignTokens.Typography.body
            value.textColor = SeyalDesignTokens.Palette.textPrimary
            value.maximumNumberOfLines = 2

            let rowStack = NSStackView(views: [label, value])
            rowStack.orientation = .vertical
            rowStack.alignment = .leading
            rowStack.spacing = 2
            stack.addArrangedSubview(rowStack)
            rowStack.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -24).isActive = true
        }

        panel.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            stack.topAnchor.constraint(equalTo: panel.topAnchor),
        ])
        return panel
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
        emphasized: Bool,
        attention: Bool
    ) -> NSView {
        let primaryField = NSTextField(labelWithString: attention ? "\(primary)  •" : primary)
        primaryField.font = emphasized
            ? SeyalDesignTokens.Typography.bodyEmphasized
            : SeyalDesignTokens.Typography.body
        primaryField.textColor = emphasized
            ? SeyalDesignTokens.Palette.textPrimary
            : SeyalDesignTokens.Palette.textSecondary
        primaryField.lineBreakMode = .byTruncatingTail

        let views: [NSView]
        if let secondary {
            let secondaryField = NSTextField(labelWithString: secondary)
            secondaryField.font = SeyalDesignTokens.Typography.metadata
            secondaryField.textColor = SeyalDesignTokens.Palette.textTertiary
            secondaryField.lineBreakMode = .byTruncatingMiddle
            views = [primaryField, secondaryField]
        } else {
            views = [primaryField]
        }

        let row = NSStackView(views: views)
        row.orientation = .vertical
        row.alignment = .leading
        row.spacing = 1
        row.translatesAutoresizingMaskIntoConstraints = false
        row.widthAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.leftContextWidth - 24).isActive = true
        return row
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

        appendSectionTitle("Attention", to: stack)
        for item in snapshot.attentionItems {
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
        controller.preferredContentSize = NSSize(width: 324, height: max(120, CGFloat(snapshot.attentionItems.count) * 66 + 44))

        let popover = NSPopover()
        popover.behavior = .transient
        popover.contentViewController = controller
        popover.show(relativeTo: sender.bounds, of: sender, preferredEdge: .maxY)
        attentionPopover = popover
    }

    static func smokeTest() -> Bool {
        #if DEBUG
        let bodies = SeyalShellPreviewData.terminalLines.map { PreviewTerminalFixtureView(lines: $0) as NSView }
        let shell = SeyalShellView(
            frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
            snapshot: SeyalShellPreviewData.snapshot,
            blocks: SeyalShellPreviewData.blocks,
            blockBodies: bodies
        )
        shell.layoutSubtreeIfNeeded()
        return shell.subviews.count == 1
        #else
        return true
        #endif
    }
}
