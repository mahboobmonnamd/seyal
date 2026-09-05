import AppKit

extension SeyalShellView {
  func makeActiveTabSurface() -> NSView {
    let container = NSView()
    container.translatesAutoresizingMaskIntoConstraints = false
    container.wantsLayer = true
    container.layer?.backgroundColor = visual.colors.ns(.canvas).cgColor

    let tree = makePaneTree(state.activeTab.root)
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

  func makePaneTree(_ node: SeyalShellState.PaneTree) -> NSView {
    switch node {
    case .pane(let paneID):
      return makePane(paneID: paneID)
    case .split(let axis, let first, let second):
      let firstView = makePaneTree(first)
      let secondView = makePaneTree(second)
      let stack = NSStackView(views: [firstView, secondView])
      stack.orientation = axis == .right ? .horizontal : .vertical
      stack.distribution = .fillEqually
      stack.spacing = 1
      stack.translatesAutoresizingMaskIntoConstraints = false
      stack.wantsLayer = true
      stack.layer?.backgroundColor = visual.colors.ns(.seamHover).cgColor
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

  func makePane(paneID: String) -> NSView {
    guard let paneState = state.activeTab.panes[paneID] else {
      preconditionFailure("Pane tree referenced a missing preview Pane")
    }
    let isFocused = state.activeTab.focusedPaneID == paneID

    let pane = NSView()
    pane.translatesAutoresizingMaskIntoConstraints = false
    pane.wantsLayer = true
    pane.layer?.backgroundColor = visual.colors.cg(.canvas)
    SeyalFocusTreatment.apply(isFocused, to: pane, visual: visual)
    paneContainers[paneID] = pane

    let focusButton = NSButton(
      title: paneState.title, target: self, action: #selector(focusPane(_:)))
    focusButton.identifier = NSUserInterfaceItemIdentifier(paneID)
    focusButton.setAccessibilityIdentifier("pane.focus.\(paneID)")
    focusButton.setAccessibilityLabel(paneState.title)
    focusButton.bezelStyle = .inline
    focusButton.isBordered = false
    focusButton.alignment = .left
    focusButton.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
    focusButton.contentTintColor = visual.colors.ns(.textPrimary)

    let type = NSTextField(labelWithString: "Terminal")
    type.font = visual.typography[.metadata]
    type.textColor = visual.colors.ns(.textMuted)

    let titleStack = NSStackView(views: [focusButton, type])
    titleStack.orientation = .vertical
    titleStack.alignment = .leading
    titleStack.spacing = 1
    titleStack.setContentHuggingPriority(.defaultHigh, for: .horizontal)

    let spacer = NSView()
    spacer.translatesAutoresizingMaskIntoConstraints = false
    spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
    spacer.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

    let focusState = NSTextField(labelWithString: isFocused ? "Focused" : "")
    focusState.font = visual.typography[.metadata]
    focusState.textColor = visual.colors.ns(.focus)
    focusState.setContentHuggingPriority(.required, for: .horizontal)
    paneFocusLabels[paneID] = focusState

    let split = makePaneControlButton(
      paneID: paneID,
      symbol: "rectangle.split.2x1",
      fallback: "Split",
      accessibilityLabel: "Split \(paneState.title)",
      accessibilityID: "pane.split.\(paneID)",
      action: #selector(showPaneSplitMenu(_:))
    )
    let close = makePaneControlButton(
      paneID: paneID,
      symbol: "xmark",
      fallback: "×",
      accessibilityLabel: "Close \(paneState.title)",
      accessibilityID: "pane.close.\(paneID)",
      action: #selector(closePane(_:))
    )
    let reconnect = makePaneControlButton(
      paneID: paneID,
      symbol: "arrow.clockwise",
      fallback: "↻",
      accessibilityLabel: "Reconnect \(paneState.title)",
      accessibilityID: "pane.reconnect.\(paneID)",
      action: #selector(reconnectPane(_:))
    )
    close.isHidden = state.activeTab.paneCount <= 1

    let controls = NSStackView(views: [split, reconnect, close])
    controls.orientation = .horizontal
    controls.alignment = .centerY
    controls.spacing = 2
    controls.setContentHuggingPriority(.required, for: .horizontal)

    let header = NSStackView(views: [titleStack, spacer, focusState, controls])
    header.orientation = .horizontal
    header.alignment = .centerY
    header.spacing = 8
    header.edgeInsets = NSEdgeInsets(top: 7, left: 10, bottom: 6, right: 8)
    header.translatesAutoresizingMaskIntoConstraints = false

    let transcript = makeTranscript(paneID: paneID)
    transcript.translatesAutoresizingMaskIntoConstraints = false
    transcript.setContentHuggingPriority(.defaultLow, for: .vertical)
    transcript.setContentCompressionResistancePriority(.defaultLow, for: .vertical)

    let composerMode: PaneComposerShellView.Mode = {
      if tuiPaneIDs.contains(paneID) { return .hiddenForTUI }
      guard let latest = paneState.blocks.last,
        latest.state == .running
      else { return .available }
      return .busy(process: latest.command)
    }()
    let composer = PaneComposerShellView(
      mode: composerMode,
      draft: paneState.draft,
      visual: visual,
      accessibilityID: "composer.\(paneID)",
      onFocus: { [weak self] in
        self?.focusPaneWithoutRebuild(paneID)
      },
      onDraftChange: { [weak self] draft in
        self?.state.updateDraft(draft, paneID: paneID)
      },
      onSubmit: { [weak self] command in
        _ = self?.submitCommand(command, paneID: paneID)
        return false
      }
    )
    composerViews[paneID] = composer
    composer.isHidden = tuiPaneIDs.contains(paneID)
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
  func setPaneTUI(paneID: String, active: Bool) {
    if active {
      tuiPaneIDs.insert(paneID)
    } else {
      tuiPaneIDs.remove(paneID)
    }
    tuiBlocks[paneID]?.setTUITakeover(active)
    composerViews[paneID]?.isHidden = active
  }

  func makePaneControlButton(
    paneID: String,
    symbol: String,
    fallback: String,
    accessibilityLabel: String,
    accessibilityID: String,
    action: Selector
  ) -> NSButton {
    let button = NSButton(title: "", target: self, action: action)
    button.identifier = NSUserInterfaceItemIdentifier(paneID)
    button.bezelStyle = .inline
    button.isBordered = false
    button.image = NSImage(systemSymbolName: symbol, accessibilityDescription: accessibilityLabel)
    if button.image == nil {
      button.title = fallback
    }
    button.contentTintColor = visual.colors.ns(.textMuted)
    button.toolTip = accessibilityLabel
    button.setAccessibilityLabel(accessibilityLabel)
    button.setAccessibilityIdentifier(accessibilityID)
    button.translatesAutoresizingMaskIntoConstraints = false
    button.widthAnchor.constraint(greaterThanOrEqualToConstant: 26).isActive = true
    button.heightAnchor.constraint(equalToConstant: 26).isActive = true
    return button
  }

  func focusPaneWithoutRebuild(_ paneID: String) {
    state.focusPane(id: paneID)
    for (id, pane) in paneContainers {
      let focused = id == state.activeTab.focusedPaneID
      SeyalFocusTreatment.apply(focused, to: pane, visual: visual)
      paneFocusLabels[id]?.stringValue = focused ? "Focused" : ""
    }
    composerView = composerViews[paneID]
    if let inspectorView, isInspectorVisible {
      inspectorView.subviews.first.map(populateInspector)
    }
  }

  @objc
  func focusPane(_ sender: NSButton) {
    guard let id = sender.identifier?.rawValue else { return }
    focusPaneWithoutRebuild(id)
  }

  @objc
  func showPaneSplitMenu(_ sender: NSButton) {
    guard let paneID = sender.identifier?.rawValue else { return }
    state.focusPane(id: paneID)

    let menu = NSMenu(title: "Split Pane")
    let splitRight = NSMenuItem(
      title: "Split Right", action: #selector(splitPaneRightFromMenu(_:)), keyEquivalent: "")
    splitRight.target = self
    splitRight.representedObject = paneID
    menu.addItem(splitRight)

    let splitDown = NSMenuItem(
      title: "Split Down", action: #selector(splitPaneDownFromMenu(_:)), keyEquivalent: "")
    splitDown.target = self
    splitDown.representedObject = paneID
    menu.addItem(splitDown)

    menu.popUp(positioning: nil, at: NSPoint(x: 0, y: sender.bounds.minY - 2), in: sender)
  }

  @objc
  func splitPaneRightFromMenu(_ sender: NSMenuItem) {
    guard let paneID = sender.representedObject as? String else { return }
    performSplit(axis: .right, paneID: paneID)
  }

  @objc
  func splitPaneDownFromMenu(_ sender: NSMenuItem) {
    guard let paneID = sender.representedObject as? String else { return }
    performSplit(axis: .down, paneID: paneID)
  }

  func performSplit(axis: SeyalShellState.SplitAxis, paneID: String) {
    guard state.splitPane(id: paneID, axis: axis) != nil else {
      presentActionError(
        state.lastActionError
          ?? "Splitting panes is unavailable until a distinct execution route is available."
      )
      return
    }
    rebuildUI()
  }

  func presentActionError(_ message: String) {
    let alert = NSAlert()
    alert.alertStyle = .informational
    alert.messageText = "Action unavailable"
    alert.informativeText = message
    alert.addButton(withTitle: "OK")
    alert.runModal()
  }

  @objc
  func closePane(_ sender: NSButton) {
    guard let paneID = sender.identifier?.rawValue else { return }
    state.closePane(id: paneID)
    rebuildUI()
  }
}
