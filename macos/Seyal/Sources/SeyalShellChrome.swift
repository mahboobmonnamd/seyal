import AppKit

extension SeyalShellView {
  func makeTopChrome() -> NSView {
    let container = NSView()
    container.translatesAutoresizingMaskIntoConstraints = false
    container.wantsLayer = true
    container.layer?.backgroundColor = visual.colors.ns(.utilityReceded).cgColor

    let leftSidebarToggle = makeToolbarButton(
      symbol: "sidebar.left",
      fallback: "☰",
      accessibilityLabel: isLeftContextVisible ? "Hide left sidebar" : "Show left sidebar",
      accessibilityID: "toggle-left-sidebar",
      action: #selector(toggleLeftSidebar(_:))
    )

    let workspaceField = NSTextField(labelWithString: state.activeWorkspace.name)
    workspaceField.font = visual.typography[.windowTitle]
    workspaceField.textColor = visual.colors.ns(.textPrimary)
    workspaceField.lineBreakMode = .byTruncatingTail
    workspaceField.translatesAutoresizingMaskIntoConstraints = false

    let slash = NSTextField(labelWithString: "/")
    slash.font = visual.typography[.tab]
    slash.textColor = visual.colors.ns(.textMuted)
    slash.translatesAutoresizingMaskIntoConstraints = false

    let tabs = makeTabStrip()
    tabs.translatesAutoresizingMaskIntoConstraints = false

    let splitRight = makeToolbarButton(
      symbol: "rectangle.split.2x1",
      fallback: "⇥",
      accessibilityLabel: "Split focused Pane right",
      accessibilityID: "split-right",
      action: #selector(splitRight(_:))
    )
    let splitDown = makeToolbarButton(
      symbol: "rectangle.split.1x2",
      fallback: "⇣",
      accessibilityLabel: "Split focused Pane down",
      accessibilityID: "split-down",
      action: #selector(splitDown(_:))
    )
    let inspectorToggle = makeToolbarButton(
      symbol: "sidebar.right",
      fallback: "▥",
      accessibilityLabel: isInspectorVisible ? "Hide Inspector" : "Show Inspector",
      accessibilityID: "toggle-inspector",
      action: #selector(toggleInspector(_:))
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

    let controls = NSStackView(views: [splitRight, splitDown, inspectorToggle, attentionButton])
    controls.orientation = .horizontal
    controls.alignment = .centerY
    controls.spacing = 4
    controls.translatesAutoresizingMaskIntoConstraints = false

    container.addSubview(leftSidebarToggle)
    container.addSubview(workspaceField)
    container.addSubview(slash)
    container.addSubview(tabs)
    container.addSubview(controls)

    NSLayoutConstraint.activate([
      leftSidebarToggle.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 8),
      leftSidebarToggle.centerYAnchor.constraint(equalTo: container.centerYAnchor),

      workspaceField.leadingAnchor.constraint(
        equalTo: leftSidebarToggle.trailingAnchor, constant: 6),
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

  func makeToolbarButton(
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
    button.contentTintColor = visual.colors.ns(.textSecondary)
    button.toolTip = accessibilityLabel
    button.setAccessibilityLabel(accessibilityLabel)
    button.setAccessibilityIdentifier(accessibilityID)
    button.translatesAutoresizingMaskIntoConstraints = false
    button.widthAnchor.constraint(greaterThanOrEqualToConstant: 28).isActive = true
    button.heightAnchor.constraint(equalToConstant: 28).isActive = true
    return button
  }

  func makeTabStrip() -> NSScrollView {
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

  func makeTabChip(_ tab: SeyalShellSnapshot.Tab) -> NSView {
    let isActive = tab.id == snapshot.activeTabID
    let button = NSButton(title: tab.title, target: self, action: #selector(selectTab(_:)))
    button.identifier = NSUserInterfaceItemIdentifier(tab.id)
    button.setAccessibilityIdentifier("tab.\(tab.id)")
    button.setAccessibilityLabel(tab.title)
    button.bezelStyle = .inline
    button.isBordered = false
    button.font =
      isActive
      ? visual.typography[.windowTitle]
      : visual.typography[.tab]
    button.contentTintColor =
      isActive
      ? visual.colors.ns(.textPrimary)
      : visual.colors.ns(.textSecondary)
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
    close.contentTintColor = visual.colors.ns(.textMuted)
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
    container.layer?.backgroundColor =
      isActive
      ? visual.colors.ns(.utilityActive).cgColor
      : NSColor.clear.cgColor
    container.addSubview(row)

    if isActive {
      let accent = NSView()
      accent.translatesAutoresizingMaskIntoConstraints = false
      accent.wantsLayer = true
      accent.layer?.backgroundColor = visual.colors.ns(.focus).cgColor
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
      button.widthAnchor.constraint(
        greaterThanOrEqualToConstant: visual.metrics.tabMinWidth - 30),
      container.widthAnchor.constraint(
        lessThanOrEqualToConstant: visual.metrics.tabMaxWidth),
    ])
    return container
  }

  func makeLeftContextPanel() -> NSView {
    let panel = NSView()
    panel.translatesAutoresizingMaskIntoConstraints = false
    panel.wantsLayer = true
    panel.layer?.backgroundColor = visual.colors.ns(.utilityReceded).cgColor

    let modeControl = SeyalPreviewModeControl(
      labels: ["Workspaces", "Tabs"],
      trackingMode: .selectOne,
      target: self,
      action: #selector(changeLeftPanelMode(_:))
    )
    modeControl.selectedSegment = state.leftPanelMode == .workspaces ? 0 : 1
    modeControl.segmentStyle = .texturedRounded
    modeControl.setAccessibilityIdentifier("left-mode")
    modeControl.setAccessibilityLabel("Left panel mode")
    modeControl.setAccessibilityRole(.radioGroup)
    modeControl.translatesAutoresizingMaskIntoConstraints = false

    let collapse = makeToolbarButton(
      symbol: "chevron.left",
      fallback: "‹",
      accessibilityLabel: "Hide left sidebar",
      accessibilityID: "left-sidebar-collapse",
      action: #selector(toggleLeftSidebar(_:))
    )
    collapse.contentTintColor = visual.colors.ns(.textMuted)

    let header = NSStackView(views: [modeControl, collapse])
    header.orientation = .horizontal
    header.alignment = .centerY
    header.spacing = 4
    header.translatesAutoresizingMaskIntoConstraints = false

    let content = NSStackView()
    content.orientation = .vertical
    content.alignment = .leading
    content.spacing = 6
    content.translatesAutoresizingMaskIntoConstraints = false

    switch state.leftPanelMode {
    case .workspaces:
      appendWorkspaceContent(to: content)
    case .tabs:
      appendTabContent(to: content)
    }

    let stack = NSStackView(views: [header, content])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 10
    stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
    stack.translatesAutoresizingMaskIntoConstraints = false

    panel.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
      stack.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
      stack.topAnchor.constraint(equalTo: panel.topAnchor),
      stack.bottomAnchor.constraint(lessThanOrEqualTo: panel.bottomAnchor),
      header.widthAnchor.constraint(
        equalToConstant: visual.metrics.leftContextWidth - 24),
      modeControl.widthAnchor.constraint(
        equalToConstant: visual.metrics.leftContextWidth - 56),
      content.widthAnchor.constraint(
        equalToConstant: visual.metrics.leftContextWidth - 24),
    ])
    return panel
  }

  func appendWorkspaceContent(to stack: NSStackView) {
    appendSectionTitle("Workspaces", to: stack)
    snapshot.workspaces.forEach { workspace in
      let count = workspace.tabCount == 1 ? "1 tab" : "\(workspace.tabCount) tabs"
      stack.addArrangedSubview(
        makeContextButton(
          primary: workspace.name,
          secondary: workspace.detail,
          trailing: count,
          emphasized: workspace.id == snapshot.activeWorkspaceID,
          attention: workspace.attention,
          statusColor: workspace.id == snapshot.activeWorkspaceID
            ? visual.colors.ns(.success)
            : nil,
          itemID: workspace.id,
          accessibilityID: "workspace.\(workspace.id)",
          action: #selector(selectWorkspace(_:))
        ))
    }

    stack.addArrangedSubview(makeSpacer(height: 8))
    appendSectionTitle("Agents · \(state.activeWorkspace.name)", to: stack)
    if snapshot.agents.isEmpty {
      stack.addArrangedSubview(makeEmptyStateRow("No active agent sessions"))
    } else {
      snapshot.agents.forEach { agent in
        stack.addArrangedSubview(
          makeContextButton(
            primary: agent.name,
            secondary: nil,
            trailing: agent.state.rawValue,
            emphasized: state.selectedAgentID == agent.id,
            attention: agent.state == .attention,
            statusColor: agentColor(agent.state),
            itemID: agent.id,
            accessibilityID: "agent.\(agent.id)",
            action: #selector(selectAgent(_:))
          ))
      }
    }
  }

  func appendTabContent(to stack: NSStackView) {
    appendSectionTitle(state.activeWorkspace.name, to: stack)
    if let path = state.activeWorkspace.detail {
      stack.addArrangedSubview(makeEmptyStateRow(path))
    }
    stack.addArrangedSubview(makeSpacer(height: 4))
    appendSectionTitle("Tabs", to: stack)

    snapshot.tabs.forEach { tab in
      let paneDetail = tab.paneCount == 1 ? "1 pane" : "\(tab.paneCount) panes"
      stack.addArrangedSubview(
        makeContextButton(
          primary: tab.title,
          secondary: nil,
          trailing: paneDetail,
          emphasized: tab.id == snapshot.activeTabID,
          attention: tab.attention,
          statusColor: tab.attention ? visual.colors.ns(.warning) : nil,
          itemID: tab.id,
          accessibilityID: "left-tab.\(tab.id)",
          action: #selector(selectTab(_:))
        ))
    }

    let newTab = NSButton(title: "+ New Tab", target: self, action: #selector(createTab(_:)))
    newTab.bezelStyle = .inline
    newTab.isBordered = false
    newTab.alignment = .left
    newTab.font = visual.typography[.action]
    newTab.contentTintColor = visual.colors.ns(.focus)
    newTab.setAccessibilityIdentifier("left-new-tab")
    newTab.translatesAutoresizingMaskIntoConstraints = false
    newTab.widthAnchor.constraint(equalToConstant: visual.metrics.leftContextWidth - 24)
      .isActive = true
    stack.addArrangedSubview(newTab)
  }

  func makeContextButton(
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
    container.layer?.backgroundColor =
      emphasized
      ? visual.colors.ns(.selection).cgColor
      : NSColor.clear.cgColor
    container.widthAnchor.constraint(
      equalToConstant: visual.metrics.leftContextWidth - 24
    ).isActive = true

    let dot = NSTextField(labelWithString: "●")
    dot.font = NSFont.systemFont(ofSize: 7, weight: .bold)
    dot.textColor =
      statusColor
      ?? (attention
        ? visual.colors.ns(.warning)
        : visual.colors.ns(.textMuted))
    dot.translatesAutoresizingMaskIntoConstraints = false
    dot.setContentHuggingPriority(.required, for: .horizontal)

    let button = NSButton(title: primary, target: self, action: action)
    button.identifier = NSUserInterfaceItemIdentifier(itemID)
    button.setAccessibilityIdentifier(accessibilityID)
    button.setAccessibilityLabel(primary)
    button.bezelStyle = .inline
    button.isBordered = false
    button.alignment = .left
    button.font =
      emphasized
      ? visual.typography[.action]
      : visual.typography[.uiBody]
    button.contentTintColor =
      emphasized
      ? visual.colors.ns(.textPrimary)
      : visual.colors.ns(.textSecondary)
    button.translatesAutoresizingMaskIntoConstraints = false

    let trailingField = NSTextField(labelWithString: trailing ?? "")
    trailingField.font = visual.typography[.metadata]
    trailingField.textColor =
      attention
      ? visual.colors.ns(.warning)
      : visual.colors.ns(.textMuted)
    trailingField.alignment = .right
    trailingField.translatesAutoresizingMaskIntoConstraints = false
    trailingField.setContentHuggingPriority(.required, for: .horizontal)

    container.addSubview(dot)
    container.addSubview(button)
    container.addSubview(trailingField)

    var constraints = [
      dot.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 7),
      dot.centerYAnchor.constraint(equalTo: button.centerYAnchor),
      button.leadingAnchor.constraint(equalTo: dot.trailingAnchor, constant: 5),
      button.topAnchor.constraint(equalTo: container.topAnchor, constant: 3),
      trailingField.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -7),
      trailingField.centerYAnchor.constraint(equalTo: button.centerYAnchor),
      button.trailingAnchor.constraint(
        lessThanOrEqualTo: trailingField.leadingAnchor, constant: -4),
    ]

    if let secondary {
      let secondaryField = NSTextField(labelWithString: secondary)
      secondaryField.font = visual.typography[.metadata]
      secondaryField.textColor = visual.colors.ns(.textMuted)
      secondaryField.lineBreakMode = .byTruncatingMiddle
      secondaryField.translatesAutoresizingMaskIntoConstraints = false
      container.addSubview(secondaryField)
      constraints.append(contentsOf: [
        secondaryField.leadingAnchor.constraint(equalTo: button.leadingAnchor),
        secondaryField.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -7),
        secondaryField.topAnchor.constraint(equalTo: button.bottomAnchor, constant: -1),
        secondaryField.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -5),
      ])
    } else {
      constraints.append(
        button.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -3))
    }

    NSLayoutConstraint.activate(constraints)
    return container
  }

  func makeEmptyStateRow(_ text: String) -> NSView {
    let field = NSTextField(labelWithString: text)
    field.font = visual.typography[.uiBody]
    field.textColor = visual.colors.ns(.textMuted)
    field.lineBreakMode = .byTruncatingMiddle
    field.translatesAutoresizingMaskIntoConstraints = false

    let container = NSView()
    container.translatesAutoresizingMaskIntoConstraints = false
    container.addSubview(field)
    container.widthAnchor.constraint(
      equalToConstant: visual.metrics.leftContextWidth - 24
    ).isActive = true
    NSLayoutConstraint.activate([
      field.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 7),
      field.trailingAnchor.constraint(lessThanOrEqualTo: container.trailingAnchor, constant: -7),
      field.topAnchor.constraint(equalTo: container.topAnchor, constant: 5),
      field.bottomAnchor.constraint(equalTo: container.bottomAnchor, constant: -5),
    ])
    return container
  }
  func makeInspector() -> NSView {
    let panel = NSView()
    panel.translatesAutoresizingMaskIntoConstraints = false
    panel.wantsLayer = true
    panel.layer?.backgroundColor = visual.colors.ns(.utilityReceded).cgColor

    let detail = NSView()
    detail.translatesAutoresizingMaskIntoConstraints = false
    detail.wantsLayer = true
    detail.layer?.backgroundColor = visual.colors.ns(.utilityReceded).cgColor
    populateInspector(detail)

    let rail = makeInspectorRail()
    panel.addSubview(detail)
    panel.addSubview(rail)

    NSLayoutConstraint.activate([
      detail.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
      detail.topAnchor.constraint(equalTo: panel.topAnchor),
      detail.bottomAnchor.constraint(equalTo: panel.bottomAnchor),
      detail.trailingAnchor.constraint(equalTo: rail.leadingAnchor),

      rail.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
      rail.topAnchor.constraint(equalTo: panel.topAnchor),
      rail.bottomAnchor.constraint(equalTo: panel.bottomAnchor),
      rail.widthAnchor.constraint(equalToConstant: visual.metrics.inspectorRailWidth),
    ])
    rail.constraints.first(where: { $0.firstAttribute == .width })?.priority = .defaultHigh
    return panel
  }

  func makeInspectorRail() -> NSView {
    let rail = NSView()
    rail.translatesAutoresizingMaskIntoConstraints = false
    rail.wantsLayer = true
    rail.layer?.backgroundColor = visual.colors.ns(.utilityReceded).cgColor

    let separator = NSView()
    separator.translatesAutoresizingMaskIntoConstraints = false
    separator.wantsLayer = true
    separator.layer?.backgroundColor = visual.colors.ns(.seamHover).cgColor
    rail.addSubview(separator)

    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .centerX
    stack.spacing = 4
    stack.translatesAutoresizingMaskIntoConstraints = false

    InspectorMode.allCases.forEach { mode in
      let button = NSButton(title: "", target: self, action: #selector(selectInspectorMode(_:)))
      button.identifier = NSUserInterfaceItemIdentifier(mode.rawValue)
      button.setAccessibilityIdentifier("inspector-mode.\(mode.rawValue)")
      button.setAccessibilityLabel("Inspector \(mode.title)")
      button.toolTip = mode.title
      button.bezelStyle = .inline
      button.isBordered = false
      button.image = NSImage(systemSymbolName: mode.symbol, accessibilityDescription: mode.title)
      if button.image == nil {
        button.title = String(mode.title.prefix(1))
      }
      button.contentTintColor =
        mode == inspectorMode
        ? visual.colors.ns(.focus)
        : visual.colors.ns(.textMuted)
      button.translatesAutoresizingMaskIntoConstraints = false
      button.wantsLayer = true
      button.layer?.cornerRadius = 6
      button.layer?.backgroundColor =
        mode == inspectorMode
        ? visual.colors.ns(.selection).cgColor
        : NSColor.clear.cgColor
      button.widthAnchor.constraint(equalToConstant: 28).isActive = true
      button.heightAnchor.constraint(equalToConstant: 28).isActive = true
      stack.addArrangedSubview(button)
    }

    rail.addSubview(stack)
    NSLayoutConstraint.activate([
      separator.leadingAnchor.constraint(equalTo: rail.leadingAnchor),
      separator.topAnchor.constraint(equalTo: rail.topAnchor),
      separator.bottomAnchor.constraint(equalTo: rail.bottomAnchor),
      separator.widthAnchor.constraint(equalToConstant: 1),
      stack.topAnchor.constraint(equalTo: rail.topAnchor, constant: 10),
      stack.centerXAnchor.constraint(equalTo: rail.centerXAnchor, constant: 0.5),
    ])
    return rail
  }

  func populateInspector(_ panel: NSView) {
    panel.subviews.forEach { $0.removeFromSuperview() }

    let title = NSTextField(labelWithString: "Inspector")
    title.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
    title.textColor = visual.colors.ns(.textPrimary)

    let spacer = NSView()
    spacer.translatesAutoresizingMaskIntoConstraints = false
    spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
    spacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

    let collapse = makeToolbarButton(
      symbol: "chevron.right",
      fallback: "›",
      accessibilityLabel: "Hide Inspector",
      accessibilityID: "inspector-collapse",
      action: #selector(toggleInspector(_:))
    )
    collapse.contentTintColor = visual.colors.ns(.textMuted)

    let titleRow = NSStackView(views: [title, spacer, collapse])
    titleRow.orientation = .horizontal
    titleRow.alignment = .centerY
    titleRow.spacing = 4
    titleRow.translatesAutoresizingMaskIntoConstraints = false

    let mode = NSTextField(labelWithString: inspectorMode.title.uppercased())
    mode.font = visual.typography[.sectionLabel]
    mode.textColor = visual.colors.ns(.focus)
    mode.setAccessibilityIdentifier("inspector-mode-label")
    mode.setAccessibilityLabel(inspectorMode.title.uppercased())

    let stack = NSStackView(views: [titleRow, mode])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8
    stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
    stack.translatesAutoresizingMaskIntoConstraints = false

    let rows = visibleInspectorRows()
    var currentSection: String?
    for row in rows {
      if currentSection != row.section {
        if currentSection != nil {
          stack.addArrangedSubview(makeSpacer(height: 5))
        }
        appendSectionTitle(row.section, to: stack)
        currentSection = row.section
      }
      stack.addArrangedSubview(makeInspectorRow(row))
    }

    if rows.isEmpty {
      let empty = NSTextField(
        wrappingLabelWithString:
          "No \(inspectorMode.title.lowercased()) context for the current selection")
      empty.font = visual.typography[.uiBody]
      empty.textColor = visual.colors.ns(.textMuted)
      empty.maximumNumberOfLines = 3
      empty.setAccessibilityIdentifier("inspector-empty")
      stack.addArrangedSubview(empty)
    }

    panel.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
      stack.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
      stack.topAnchor.constraint(equalTo: panel.topAnchor),
      stack.bottomAnchor.constraint(lessThanOrEqualTo: panel.bottomAnchor),
      titleRow.widthAnchor.constraint(equalToConstant: inspectorDetailWidth - 24),
    ])
  }

  var inspectorDetailWidth: CGFloat {
    visual.metrics.inspectorWidth - visual.metrics.inspectorRailWidth
  }

  func visibleInspectorRows() -> [SeyalShellSnapshot.InspectorRow] {
    switch inspectorMode {
    case .context:
      snapshot.inspectorRows
    case .workspace:
      snapshot.inspectorRows.filter { $0.section == "Workspace" }
    case .tab:
      snapshot.inspectorRows.filter { $0.section == "Tab" }
    case .pane:
      snapshot.inspectorRows.filter { $0.section == "Active Pane" }
    }
  }

  func makeInspectorRow(_ row: SeyalShellSnapshot.InspectorRow) -> NSView {
    let label = NSTextField(labelWithString: row.label)
    label.font = visual.typography[.metadata]
    label.textColor = visual.colors.ns(.textMuted)
    label.setContentCompressionResistancePriority(.required, for: .horizontal)

    let value = NSTextField(labelWithString: row.value)
    value.font = visual.typography[.uiBody]
    value.textColor = visual.colors.ns(.textPrimary)
    value.lineBreakMode = .byTruncatingMiddle
    value.alignment = .right
    value.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    value.setAccessibilityIdentifier("inspector.\(row.id)")
    value.setAccessibilityLabel(row.value)

    let rowStack = NSStackView(views: [label, value])
    rowStack.orientation = .horizontal
    rowStack.alignment = .centerY
    rowStack.distribution = .fill
    rowStack.spacing = 8
    rowStack.translatesAutoresizingMaskIntoConstraints = false
    rowStack.widthAnchor.constraint(equalToConstant: inspectorDetailWidth - 24).isActive = true
    return rowStack
  }

  func appendSectionTitle(_ title: String, to stack: NSStackView) {
    let field = NSTextField(labelWithString: title.uppercased())
    field.font = visual.typography[.sectionLabel]
    field.textColor = visual.colors.ns(.textMuted)
    stack.addArrangedSubview(field)
  }

  func agentColor(_ agentState: SeyalShellSnapshot.Agent.State) -> NSColor {
    switch agentState {
    case .running:
      visual.colors.ns(.success)
    case .waiting:
      visual.colors.ns(.information)
    case .attention:
      visual.colors.ns(.warning)
    case .idle:
      visual.colors.ns(.textMuted)
    }
  }

  func makeSpacer(height: CGFloat) -> NSView {
    let spacer = NSView()
    spacer.translatesAutoresizingMaskIntoConstraints = false
    spacer.heightAnchor.constraint(equalToConstant: height).isActive = true
    return spacer
  }
  @objc
  func changeLeftPanelMode(_ sender: NSSegmentedControl) {
    state.setLeftPanelMode(sender.selectedSegment == 0 ? .workspaces : .tabs)
    rebuildUI()
  }

  @objc
  func toggleLeftSidebar(_ sender: NSButton) {
    isLeftContextVisible.toggle()
    rebuildUI()
  }

  @objc
  func toggleInspector(_ sender: NSButton) {
    isInspectorVisible.toggle()
    rebuildUI()
  }

  @objc
  func selectInspectorMode(_ sender: NSButton) {
    guard let rawValue = sender.identifier?.rawValue,
      let mode = InspectorMode(rawValue: rawValue)
    else {
      return
    }
    inspectorMode = mode
    rebuildUI()
  }

  @objc
  func selectWorkspace(_ sender: NSButton) {
    guard let id = sender.identifier?.rawValue else { return }
    state.selectWorkspace(id: id)
    rebuildUI()
  }

  @objc
  func selectTab(_ sender: NSButton) {
    guard let id = sender.identifier?.rawValue else { return }
    state.selectTab(id: id)
    rebuildUI()
  }

  @objc
  func selectAgent(_ sender: NSButton) {
    guard let id = sender.identifier?.rawValue else { return }
    state.selectAgent(id: id)
    rebuildUI()
  }

  @objc
  func createTab(_ sender: NSButton) {
    guard state.createTab() != nil else {
      presentActionError(
        state.lastActionError
          ?? "Creating tabs is unavailable until a distinct execution route is available."
      )
      return
    }
    state.setLeftPanelMode(.tabs)
    rebuildUI()
  }

  @objc
  func closeTab(_ sender: NSButton) {
    guard let id = sender.identifier?.rawValue else { return }
    state.closeTab(id: id)
    rebuildUI()
  }

  @objc
  func splitRight(_ sender: NSButton) {
    performSplit(axis: .right, paneID: state.activeTab.focusedPaneID)
  }

  @objc
  func splitDown(_ sender: NSButton) {
    performSplit(axis: .down, paneID: state.activeTab.focusedPaneID)
  }

  @objc
  func showAttention(_ sender: NSButton) {
    let stack = NSStackView()
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 10
    stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
    stack.translatesAutoresizingMaskIntoConstraints = false
    stack.wantsLayer = true
    stack.layer?.backgroundColor = visual.colors.ns(.utilityActive).cgColor

    appendSectionTitle("Attention", to: stack)
    if snapshot.attentionItems.isEmpty {
      let empty = NSTextField(labelWithString: "No attention items")
      empty.font = visual.typography[.uiBody]
      empty.textColor = visual.colors.ns(.textMuted)
      stack.addArrangedSubview(empty)
    } else {
      snapshot.attentionItems.forEach { item in
        let button = NSButton(
          title: item.title, target: self, action: #selector(openAttentionItem(_:)))
        button.identifier = NSUserInterfaceItemIdentifier(item.id)
        button.setAccessibilityIdentifier("attention-item.\(item.id)")
        button.bezelStyle = .inline
        button.isBordered = false
        button.alignment = .left
        button.font = visual.typography[.action]
        button.contentTintColor = visual.colors.ns(.textPrimary)

        let detail = NSTextField(wrappingLabelWithString: item.detail)
        detail.font = visual.typography[.uiBody]
        detail.textColor = visual.colors.ns(.textSecondary)
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
  func openAttentionItem(_ sender: NSButton) {
    guard let id = sender.identifier?.rawValue else { return }
    state.openAttentionItem(id: id)
    rebuildUI()
  }
}
