import AppKit

#if DEBUG
@MainActor
final class SeyalShellView: NSView {
    struct LayoutContract {
        let topChrome: NSRect
        let leftContext: NSRect
        let pane: NSRect
        let inspector: NSRect
        let composer: NSRect
    }

    private let state: SeyalShellPreviewState
    private var attentionPopover: NSPopover?
    private var paneContainers: [String: NSView] = [:]
    private var composerViews: [String: PaneComposerShellView] = [:]

    private weak var topChromeView: NSView?
    private weak var leftContextView: NSView?
    private weak var paneView: NSView?
    private weak var inspectorView: NSView?
    private weak var composerView: NSView?

    private var snapshot: SeyalShellSnapshot { state.snapshot }

    init(frame frameRect: NSRect, state: SeyalShellPreviewState) {
        self.state = state
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = SeyalDesignTokens.Palette.windowBackground.cgColor
        buildUI()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("SeyalShellView is programmatic")
    }

    private func rebuildUI() {
        attentionPopover?.close()
        attentionPopover = nil
        paneContainers.removeAll()
        composerViews.removeAll()
        subviews.forEach { $0.removeFromSuperview() }
        buildUI()
        needsLayout = true
        layoutSubtreeIfNeeded()
    }

    private func buildUI() {
        let topChrome = makeTopChrome()
        let leftContext = makeLeftContextPanel()
        let pane = makeActiveTabSurface()
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

        let workspaceField = NSTextField(labelWithString: state.activeWorkspace.name)
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

        let splitRight = makeToolbarButton(
            symbol: "rectangle.split.2x1",
            fallback: "⇥",
            accessibilityLabel: "Split Right",
            accessibilityID: "split-right",
            action: #selector(splitRight(_:))
        )
        let splitDown = makeToolbarButton(
            symbol: "rectangle.split.1x2",
            fallback: "⇣",
            accessibilityLabel: "Split Down",
            accessibilityID: "split-down",
            action: #selector(splitDown(_:))
        )
        let attentionButton = makeToolbarButton(
            symbol: snapshot.attentionItems.isEmpty ? "bell" : "bell.badge",
            fallback: "!",
            accessibilityLabel: "Attention",
            accessibilityID: "attention",
            action: #selector(showAttention(_:))
        )
        if !snapshot.attentionItems.isEmpty {
            attentionButton.title = " \(snapshot.attentionItems.count)"
            attentionButton.imagePosition = .imageLeading
        }

        let controls = NSStackView(views: [splitRight, splitDown, attentionButton])
        controls.orientation = .horizontal
        controls.alignment = .centerY
        controls.spacing = 4
        controls.translatesAutoresizingMaskIntoConstraints = false

        container.addSubview(workspaceField)
        container.addSubview(slash)
        container.addSubview(tabs)
        container.addSubview(controls)

        NSLayoutConstraint.activate([
            workspaceField.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 14),
            workspaceField.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            workspaceField.widthAnchor.constraint(lessThanOrEqualToConstant: 150),

            slash.leadingAnchor.constraint(equalTo: workspaceField.trailingAnchor, constant: 7),
            slash.centerYAnchor.constraint(equalTo: container.centerYAnchor),

            tabs.leadingAnchor.constraint(equalTo: slash.trailingAnchor, constant: 8),
            tabs.trailingAnchor.constraint(equalTo: controls.leadingAnchor, constant: -8),
            tabs.topAnchor.constraint(equalTo: container.topAnchor, constant: 4),
            tabs.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -4),

            controls.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -10),
            controls.centerYAnchor.constraint(equalTo: container.centerYAnchor),
        ])

        return container
    }

    private func makeToolbarButton(
        symbol: String,
        fallback: String,
        accessibilityLabel: String,
        accessibilityID: String,
        action: Selector
    ) -> NSButton {
        let button = NSButton(title: "", target: self, action: action)
        button.bezelStyle = .inline
        button.isBordered = false
        button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: accessibilityLabel)
        if button.image == nil {
            button.title = fallback
        }
        button.contentTintColor = SeyalDesignTokens.Palette.textSecondary
        button.toolTip = accessibilityLabel
        button.setAccessibilityLabel(accessibilityLabel)
        button.setAccessibilityIdentifier(accessibilityID)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.widthAnchor.constraint(greaterThanOrEqualToConstant: 28).isActive = true
        button.heightAnchor.constraint(equalToConstant: 28).isActive = true
        return button
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

        let newTab = makeToolbarButton(
            symbol: "plus",
            fallback: "+",
            accessibilityLabel: "New Tab",
            accessibilityID: "new-tab",
            action: #selector(createTab(_:))
        )
        tabStack.addArrangedSubview(newTab)

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
        let isActive = tab.id == snapshot.activeTabID
        let button = NSButton(title: tab.title, target: self, action: #selector(selectTab(_:)))
        button.identifier = NSUserInterfaceItemIdentifier(tab.id)
        button.setAccessibilityIdentifier("tab.\(tab.id)")
        button.setAccessibilityLabel(tab.title)
        button.bezelStyle = .inline
        button.isBordered = false
        button.font = isActive
            ? SeyalDesignTokens.Typography.chromeEmphasized
            : SeyalDesignTokens.Typography.chrome
        button.contentTintColor = isActive
            ? SeyalDesignTokens.Palette.textPrimary
            : SeyalDesignTokens.Palette.textSecondary
        button.alignment = .left
        button.image = NSImage(systemSymbolName: "circle.fill", accessibilityDescription: nil)
        button.imagePosition = .imageLeading
        button.imageScaling = .scaleProportionallyDown
        button.translatesAutoresizingMaskIntoConstraints = false

        let close = NSButton(title: "", target: self, action: #selector(closeTab(_:)))
        close.identifier = NSUserInterfaceItemIdentifier(tab.id)
        close.setAccessibilityIdentifier("tab.close.\(tab.id)")
        close.setAccessibilityLabel("Close \(tab.title)")
        close.bezelStyle = .inline
        close.isBordered = false
        close.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close")
        close.contentTintColor = SeyalDesignTokens.Palette.textTertiary
        close.isHidden = snapshot.tabs.count <= 1
        close.translatesAutoresizingMaskIntoConstraints = false
        close.widthAnchor.constraint(equalToConstant: 18).isActive = true

        let row = NSStackView(views: [button, close])
        row.orientation = .horizontal
        row.alignment = .centerY
        row.spacing = 2
        row.translatesAutoresizingMaskIntoConstraints = false

        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        container.layer?.cornerRadius = 7
        container.layer?.backgroundColor = isActive
            ? SeyalDesignTokens.Palette.elevatedBackground.cgColor
            : NSColor.clear.cgColor
        container.addSubview(row)

        if isActive {
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
            row.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 8),
            row.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -6),
            row.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            button.widthAnchor.constraint(greaterThanOrEqualToConstant: SeyalDesignTokens.Layout.tabMinWidth - 30),
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
            stack.addArrangedSubview(makeContextButton(
                primary: workspace.name,
                secondary: workspace.detail,
                trailing: count,
                emphasized: workspace.id == snapshot.activeWorkspaceID,
                attention: workspace.attention,
                statusColor: workspace.id == snapshot.activeWorkspaceID
                    ? SeyalDesignTokens.Palette.success
                    : nil,
                itemID: workspace.id,
                accessibilityID: "workspace.\(workspace.id)",
                action: #selector(selectWorkspace(_:))
            ))
        }

        stack.addArrangedSubview(makeSpacer(height: 8))
        appendSectionTitle("Agents", to: stack)
        if snapshot.agents.isEmpty {
            stack.addArrangedSubview(makeEmptyStateRow("No agent sessions"))
        } else {
            snapshot.agents.forEach { agent in
                stack.addArrangedSubview(makeContextButton(
                    primary: agent.name,
                    secondary: nil,
                    trailing: agent.state.rawValue,
                    emphasized: false,
                    attention: agent.state == .attention,
                    statusColor: agentColor(agent.state),
                    itemID: agent.id,
                    accessibilityID: "agent.\(agent.id)",
                    action: #selector(selectAgent(_:))
                ))
            }
        }

        stack.addArrangedSubview(makeSpacer(height: 8))
        appendSectionTitle("Tabs", to: stack)
        snapshot.tabs.forEach { tab in
            let paneDetail = tab.paneCount == 1 ? "1 pane" : "\(tab.paneCount) panes"
            stack.addArrangedSubview(makeContextButton(
                primary: tab.title,
                secondary: nil,
                trailing: paneDetail,
                emphasized: tab.id == snapshot.activeTabID,
                attention: tab.attention,
                statusColor: tab.attention ? SeyalDesignTokens.Palette.warning : nil,
                itemID: tab.id,
                accessibilityID: "left-tab.\(tab.id)",
                action: #selector(selectTab(_:))
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

    private func makeContextButton(
        primary: String,
        secondary: String?,
        trailing: String?,
        emphasized: Bool,
        attention: Bool,
        statusColor: NSColor?,
        itemID: String,
        accessibilityID: String,
        action: Selector
    ) -> NSView {
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        container.layer?.cornerRadius = 7
        container.layer?.backgroundColor = emphasized
            ? SeyalDesignTokens.Palette.focusSoft.cgColor
            : NSColor.clear.cgColor
        container.widthAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.leftContextWidth - 24).isActive = true

        let button = NSButton(title: primary, target: self, action: action)
        button.identifier = NSUserInterfaceItemIdentifier(itemID)
        button.setAccessibilityIdentifier(accessibilityID)
        button.setAccessibilityLabel(primary)
        button.bezelStyle = .inline
        button.isBordered = false
        button.alignment = .left
        button.font = emphasized
            ? SeyalDesignTokens.Typography.bodyEmphasized
            : SeyalDesignTokens.Typography.body
        button.contentTintColor = emphasized
            ? SeyalDesignTokens.Palette.textPrimary
            : SeyalDesignTokens.Palette.textSecondary
        button.image = NSImage(systemSymbolName: "circle.fill", accessibilityDescription: nil)
        button.imagePosition = .imageLeading
        button.translatesAutoresizingMaskIntoConstraints = false

        let trailingField = NSTextField(labelWithString: trailing ?? "")
        trailingField.font = SeyalDesignTokens.Typography.metadata
        trailingField.textColor = attention
            ? SeyalDesignTokens.Palette.warning
            : SeyalDesignTokens.Palette.textTertiary
        trailingField.alignment = .right
        trailingField.translatesAutoresizingMaskIntoConstraints = false
        trailingField.setContentHuggingPriority(.required, for: .horizontal)

        container.addSubview(button)
        container.addSubview(trailingField)

        var constraints = [
            button.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 5),
            button.topAnchor.constraint(equalTo: container.topAnchor, constant: 3),
            trailingField.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -7),
            trailingField.centerYAnchor.constraint(equalTo: button.centerYAnchor),
            button.trailingAnchor.constraint(lessThanOrEqualTo: trailingField.leadingAnchor, constant: -4),
        ]

        if let secondary {
            let secondaryField = NSTextField(labelWithString: secondary)
            secondaryField.font = SeyalDesignTokens.Typography.metadata
            secondaryField.textColor = SeyalDesignTokens.Palette.textTertiary
            secondaryField.lineBreakMode = .byTruncatingMiddle
            secondaryField.translatesAutoresizingMaskIntoConstraints = false
            container.addSubview(secondaryField)
            constraints.append(contentsOf: [
                secondaryField.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 27),
                secondaryField.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -7),
                secondaryField.topAnchor.constraint(equalTo: button.bottomAnchor, constant: -1),
                secondaryField.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -5),
            ])
        } else {
            constraints.append(button.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -3))
        }

        NSLayoutConstraint.activate(constraints)
        if let statusColor {
            button.contentTintColor = emphasized ? SeyalDesignTokens.Palette.textPrimary : statusColor
        }
        return container
    }

    private func makeEmptyStateRow(_ text: String) -> NSView {
        let field = NSTextField(labelWithString: text)
        field.font = SeyalDesignTokens.Typography.body
        field.textColor = SeyalDesignTokens.Palette.textTertiary
        field.translatesAutoresizingMaskIntoConstraints = false
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(field)
        container.widthAnchor.constraint(equalToConstant: SeyalDesignTokens.Layout.leftContextWidth - 24).isActive = true
        NSLayoutConstraint.activate([
            field.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 7),
            field.trailingAnchor.constraint(lessThanOrEqualTo: container.trailingAnchor, constant: -7),
            field.topAnchor.constraint(equalTo: container.topAnchor, constant: 5),
            field.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -5),
        ])
        return container
    }

    private func makeActiveTabSurface() -> NSView {
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        container.layer?.backgroundColor = SeyalDesignTokens.Palette.paneBackground.cgColor

        let tab = state.activeTab
        let tree = makePaneTree(tab.root)
        tree.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(tree)
        NSLayoutConstraint.activate([
            tree.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 8),
            tree.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -8),
            tree.topAnchor.constraint(equalTo: container.topAnchor, constant: 8),
            tree.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -8),
        ])
        return container
    }

    private func makePaneTree(_ node: SeyalShellPreviewState.PaneTree) -> NSView {
        switch node {
        case let .pane(paneID):
            return makePane(paneID: paneID)
        case let .split(axis, first, second):
            let firstView = makePaneTree(first)
            let secondView = makePaneTree(second)
            let stack = NSStackView(views: [firstView, secondView])
            stack.orientation = axis == .right ? .horizontal : .vertical
            stack.distribution = .fillEqually
            stack.spacing = 1
            stack.translatesAutoresizingMaskIntoConstraints = false
            stack.wantsLayer = true
            stack.layer?.backgroundColor = SeyalDesignTokens.Palette.separator.cgColor
            if axis == .right {
                firstView.heightAnchor.constraint(equalTo: stack.heightAnchor).isActive = true
                secondView.heightAnchor.constraint(equalTo: stack.heightAnchor).isActive = true
            } else {
                firstView.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
                secondView.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
            }
            return stack
        }
    }

    private func makePane(paneID: String) -> NSView {
        guard let paneState = state.activeTab.panes[paneID] else {
            preconditionFailure("Pane tree referenced a missing preview Pane")
        }
        let isFocused = state.activeTab.focusedPaneID == paneID

        let pane = NSView()
        pane.translatesAutoresizingMaskIntoConstraints = false
        pane.wantsLayer = true
        pane.layer?.cornerRadius = SeyalDesignTokens.Layout.paneCornerRadius
        pane.layer?.borderWidth = isFocused ? 1.25 : 1
        pane.layer?.borderColor = (isFocused
            ? SeyalDesignTokens.Palette.focus
            : SeyalDesignTokens.Palette.separator).cgColor
        pane.layer?.backgroundColor = SeyalDesignTokens.Palette.paneBackground.cgColor
        paneContainers[paneID] = pane

        let focusButton = NSButton(title: paneState.title, target: self, action: #selector(focusPane(_:)))
        focusButton.identifier = NSUserInterfaceItemIdentifier(paneID)
        focusButton.setAccessibilityIdentifier("pane.focus.\(paneID)")
        focusButton.setAccessibilityLabel(paneState.title)
        focusButton.bezelStyle = .inline
        focusButton.isBordered = false
        focusButton.alignment = .left
        focusButton.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        focusButton.contentTintColor = SeyalDesignTokens.Palette.textPrimary

        let type = NSTextField(labelWithString: "Terminal")
        type.font = SeyalDesignTokens.Typography.metadata
        type.textColor = SeyalDesignTokens.Palette.textTertiary

        let titleStack = NSStackView(views: [focusButton, type])
        titleStack.orientation = .vertical
        titleStack.alignment = .leading
        titleStack.spacing = 1

        let focusState = NSTextField(labelWithString: isFocused ? "Focused" : "")
        focusState.font = SeyalDesignTokens.Typography.metadata
        focusState.textColor = SeyalDesignTokens.Palette.focus

        let header = NSStackView(views: [titleStack, focusState])
        header.orientation = .horizontal
        header.alignment = .centerY
        header.spacing = 8
        header.edgeInsets = NSEdgeInsets(top: 7, left: 10, bottom: 6, right: 10)
        header.translatesAutoresizingMaskIntoConstraints = false

        let transcript = makeTranscript(paneID: paneID)
        transcript.translatesAutoresizingMaskIntoConstraints = false
        transcript.setContentHuggingPriority(.defaultLow, for: .vertical)
        transcript.setContentCompressionResistancePriority(.defaultLow, for: .vertical)

        let composer = PaneComposerShellView(
            mode: .available,
            draft: paneState.draft,
            accessibilityID: "composer.\(paneID)",
            onFocus: { [weak self] in
                self?.focusPaneWithoutRebuild(paneID)
            },
            onDraftChange: { [weak self] draft in
                self?.state.updateDraft(draft, paneID: paneID)
            }
        )
        composerViews[paneID] = composer
        if isFocused {
            composerView = composer
        }

        let stack = NSStackView(views: [header, transcript, composer])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 7
        stack.edgeInsets = NSEdgeInsets(top: 0, left: 7, bottom: 7, right: 7)
        stack.translatesAutoresizingMaskIntoConstraints = false
        pane.addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: pane.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: pane.trailingAnchor),
            stack.topAnchor.constraint(equalTo: pane.topAnchor),
            stack.bottomAnchor.constraint(equalTo: pane.bottomAnchor),
            header.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            transcript.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            transcript.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            transcript.heightAnchor.constraint(greaterThanOrEqualToConstant: 180),
            composer.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            composer.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
        ])
        return pane
    }

    /// Each Pane owns one normal transcript scroll surface. The preview deliberately
    /// contains no fabricated command/output. The permanent Metal host is visible,
    /// but no TerminalExecution or display state is attached before Pass 6.
    private func makeTranscript(paneID: String) -> NSScrollView {
        let scroll = NSScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        scroll.setAccessibilityIdentifier("transcript.\(paneID)")

        let document = NSView()
        document.translatesAutoresizingMaskIntoConstraints = false
        document.wantsLayer = true
        document.layer?.backgroundColor = SeyalDesignTokens.Palette.paneBackground.cgColor

        let host = TerminalSurfaceHostView(frame: .zero)
        host.translatesAutoresizingMaskIntoConstraints = false
        document.addSubview(host)

        let title = NSTextField(labelWithString: "No TerminalExecution attached")
        title.font = SeyalDesignTokens.Typography.bodyEmphasized
        title.textColor = SeyalDesignTokens.Palette.textSecondary
        title.alignment = .center

        let detail = NSTextField(labelWithString: "UI preview only · terminal authority remains unwired until Pass 6")
        detail.font = SeyalDesignTokens.Typography.metadata
        detail.textColor = SeyalDesignTokens.Palette.textTertiary
        detail.alignment = .center

        let empty = NSStackView(views: [title, detail])
        empty.orientation = .vertical
        empty.alignment = .centerX
        empty.spacing = 4
        empty.translatesAutoresizingMaskIntoConstraints = false
        document.addSubview(empty)
        scroll.documentView = document

        NSLayoutConstraint.activate([
            host.leadingAnchor.constraint(equalTo: document.leadingAnchor),
            host.trailingAnchor.constraint(equalTo: document.trailingAnchor),
            host.topAnchor.constraint(equalTo: document.topAnchor),
            host.bottomAnchor.constraint(equalTo: document.bottomAnchor),
            empty.centerXAnchor.constraint(equalTo: document.centerXAnchor),
            empty.centerYAnchor.constraint(equalTo: document.centerYAnchor),
            document.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor),
            document.heightAnchor.constraint(greaterThanOrEqualTo: scroll.contentView.heightAnchor),
        ])
        return scroll
    }

    private func makeInspector() -> NSView {
        let panel = NSView()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.wantsLayer = true
        panel.layer?.backgroundColor = SeyalDesignTokens.Palette.panelBackground.cgColor
        populateInspector(panel)
        return panel
    }

    private func populateInspector(_ panel: NSView) {
        panel.subviews.forEach { $0.removeFromSuperview() }

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
        value.setAccessibilityIdentifier("inspector.\(row.id)")

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

    private func focusPaneWithoutRebuild(_ paneID: String) {
        state.focusPane(id: paneID)
        for (id, pane) in paneContainers {
            let focused = id == state.activeTab.focusedPaneID
            pane.layer?.borderWidth = focused ? 1.25 : 1
            pane.layer?.borderColor = (focused
                ? SeyalDesignTokens.Palette.focus
                : SeyalDesignTokens.Palette.separator).cgColor
        }
        composerView = composerViews[paneID]
        if let inspectorView {
            populateInspector(inspectorView)
        }
    }

    @objc
    private func selectWorkspace(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        state.selectWorkspace(id: id)
        rebuildUI()
    }

    @objc
    private func selectTab(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        state.selectTab(id: id)
        rebuildUI()
    }

    @objc
    private func selectAgent(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        state.selectAgent(id: id)
        rebuildUI()
    }

    @objc
    private func createTab(_ sender: NSButton) {
        state.createTab()
        rebuildUI()
    }

    @objc
    private func closeTab(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        state.closeTab(id: id)
        rebuildUI()
    }

    @objc
    private func splitRight(_ sender: NSButton) {
        state.splitFocusedPane(axis: .right)
        rebuildUI()
    }

    @objc
    private func splitDown(_ sender: NSButton) {
        state.splitFocusedPane(axis: .down)
        rebuildUI()
    }

    @objc
    private func focusPane(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        focusPaneWithoutRebuild(id)
    }

    @objc
    private func showAttention(_ sender: NSButton) {
        let stack = NSStackView()
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.wantsLayer = true
        stack.layer?.backgroundColor = SeyalDesignTokens.Palette.elevatedBackground.cgColor

        appendSectionTitle("Attention", to: stack)
        if snapshot.attentionItems.isEmpty {
            let empty = NSTextField(labelWithString: "No attention items")
            empty.font = SeyalDesignTokens.Typography.body
            empty.textColor = SeyalDesignTokens.Palette.textTertiary
            stack.addArrangedSubview(empty)
        } else {
            snapshot.attentionItems.forEach { item in
                let button = NSButton(title: item.title, target: self, action: #selector(openAttentionItem(_:)))
                button.identifier = NSUserInterfaceItemIdentifier(item.id)
                button.setAccessibilityIdentifier("attention-item.\(item.id)")
                button.bezelStyle = .inline
                button.isBordered = false
                button.alignment = .left
                button.font = SeyalDesignTokens.Typography.bodyEmphasized
                button.contentTintColor = SeyalDesignTokens.Palette.textPrimary

                let detail = NSTextField(wrappingLabelWithString: item.detail)
                detail.font = SeyalDesignTokens.Typography.body
                detail.textColor = SeyalDesignTokens.Palette.textSecondary
                detail.maximumNumberOfLines = 2

                let itemStack = NSStackView(views: [button, detail])
                itemStack.orientation = .vertical
                itemStack.alignment = .leading
                itemStack.spacing = 2
                stack.addArrangedSubview(itemStack)
                itemStack.widthAnchor.constraint(equalToConstant: 300).isActive = true
            }
        }

        let controller = NSViewController()
        controller.view = stack
        controller.preferredContentSize = NSSize(
            width: 324,
            height: snapshot.attentionItems.isEmpty
                ? 100
                : max(CGFloat(120), CGFloat(snapshot.attentionItems.count) * 66 + 44)
        )

        let popover = NSPopover()
        popover.behavior = .transient
        popover.appearance = NSAppearance(named: .darkAqua)
        popover.contentViewController = controller
        popover.show(relativeTo: sender.bounds, of: sender, preferredEdge: .maxY)
        attentionPopover = popover
    }

    @objc
    private func openAttentionItem(_ sender: NSButton) {
        guard let id = sender.identifier?.rawValue else { return }
        state.openAttentionItem(id: id)
        rebuildUI()
    }

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

    func debugSnapshot() -> SeyalShellSnapshot {
        state.snapshot
    }

    static func smokeTest() -> Bool {
        let shell = SeyalShellPreviewFactory.make(frame: NSRect(x: 0, y: 0, width: 1280, height: 800))
        shell.layoutSubtreeIfNeeded()
        guard let contract = shell.debugLayoutContract() else { return false }
        return shell.subviews.count == 1
            && abs(contract.topChrome.height - SeyalDesignTokens.Layout.topChromeHeight) < 1
            && abs(contract.leftContext.width - SeyalDesignTokens.Layout.leftContextWidth) < 1
            && abs(contract.inspector.width - SeyalDesignTokens.Layout.inspectorWidth) < 1
            && contract.pane.width > 600
    }
}
#endif
