import AppKit

/// Owns constraints created for dynamic Block views by stable Block identity.
/// A timeline eviction removes exactly the constraints associated with that
/// identity; constraints for retained Blocks remain active and untouched.
@MainActor
final class KeyedConstraintOwnership {
  private var constraintsByKey: [PaneBlockKey: [NSLayoutConstraint]] = [:]

  var count: Int { constraintsByKey.count }

  func install(_ constraints: [NSLayoutConstraint], for key: PaneBlockKey) {
    guard !constraints.isEmpty else { return }
    remove(key)
    constraintsByKey[key] = constraints
    NSLayoutConstraint.activate(constraints)
  }

  func contains(_ key: PaneBlockKey) -> Bool {
    constraintsByKey[key] != nil
  }

  func remove(_ key: PaneBlockKey) {
    guard let constraints = constraintsByKey.removeValue(forKey: key) else { return }
    NSLayoutConstraint.deactivate(constraints)
  }

  func removeAll() {
    let constraints = constraintsByKey.values.flatMap { $0 }
    constraintsByKey.removeAll()
    NSLayoutConstraint.deactivate(constraints)
  }
}

private final class SeyalPreviewModeControl: NSSegmentedControl {
  override func accessibilityRole() -> NSAccessibility.Role? { .radioGroup }
  override func isAccessibilityElement() -> Bool { true }
}

@MainActor
final class SeyalShellView: NSView {
  enum InspectorMode: String, CaseIterable {
    case context
    case workspace
    case tab
    case pane

    var title: String {
      switch self {
      case .context: "Context"
      case .workspace: "Workspace"
      case .tab: "Tab"
      case .pane: "Pane"
      }
    }

    var symbol: String {
      switch self {
      case .context: "info.circle"
      case .workspace: "folder"
      case .tab: "rectangle.on.rectangle"
      case .pane: "rectangle.split.2x1"
      }
    }
  }

  struct LayoutContract {
    let topChrome: NSRect
    let leftContext: NSRect
    let pane: NSRect
    let inspector: NSRect
    let composer: NSRect
  }

  let state: SeyalShellState
  let productionShell: Bool
  var visual: SeyalResolvedVisualConfiguration
  var attentionPopover: NSPopover?
  var paneContainers: [String: NSView] = [:]
  var composerViews: [String: PaneComposerShellView] = [:]
  var tuiBlocks: [String: BlockView] = [:]
  var surfaces: [String: InteractiveMetalSurfaceView] = [:]
  var transcriptDocuments: [String: PaneTranscriptView] = [:]
  var blockStacks: [String: NSStackView] = [:]
  var tuiPaneIDs: Set<String> = []
  var pendingComposerRequests: [String: UInt64] = [:]
  var requestedHistoryBlocks: Set<PaneBlockKey> = []
  var renderedBlockIDs: [String: [PaneBlockKey]] = [:]
  var blockBodies: [PaneBlockKey: CommandBlockBodyView] = [:]
  var blockViews: [PaneBlockKey: BlockView] = [:]
  let blockConstraintOwnership = KeyedConstraintOwnership()
  var paneFocusLabels: [String: NSTextField] = [:]
  var isLeftContextVisible = true
  var isInspectorVisible = true
  var inspectorMode: InspectorMode = .context

  weak var topChromeView: NSView?
  weak var leftContextView: NSView?
  weak var paneView: NSView?
  weak var inspectorView: NSView?
  weak var composerView: NSView?

  var snapshot: SeyalShellSnapshot { state.snapshot }

  init(
    frame frameRect: NSRect,
    state: SeyalShellState,
    productionShell: Bool = false,
    visual: SeyalResolvedVisualConfiguration
  ) {
    self.state = state
    self.productionShell = productionShell
    self.visual = visual
    super.init(frame: frameRect)
    wantsLayer = true
    layer?.backgroundColor = visual.colors.cg(.container)
    buildUI()
  }

  /// Re-resolve chrome from the authoritative visual snapshot.
  ///
  /// Presentation views are rebuilt so typography/colour/material stay
  /// consistent with the snapshot. This does not recreate TerminalExecution,
  /// VT, PTY, or Runtime ownership — only AppKit chrome/surfaces that consume
  /// the already-resolved tokens.
  func applyVisualConfiguration(_ visual: SeyalResolvedVisualConfiguration) {
    let appearanceChanged = self.visual.appearance != visual.appearance
      || self.visual.reduceTransparency != visual.reduceTransparency
      || self.visual.colors[.textPrimary] != visual.colors[.textPrimary]
      || self.visual.colors[.canvas] != visual.colors[.canvas]
      || self.visual.metrics != visual.metrics
      || self.visual.uiFont != visual.uiFont
      || self.visual.terminalFont != visual.terminalFont
    self.visual = visual
    layer?.backgroundColor = visual.colors.cg(.container)
    guard appearanceChanged else { return }
    rebuildUI()
    layer?.backgroundColor = visual.colors.cg(.container)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) {
    fatalError("SeyalShellView is programmatic")
  }

  func rebuildUI() {
    attentionPopover?.close()
    attentionPopover = nil
    paneContainers.removeAll()
    composerViews.removeAll()
    paneFocusLabels.removeAll()
    tuiBlocks.removeAll()
    surfaces.removeAll()
    transcriptDocuments.removeAll()
    blockStacks.removeAll()
    blockViews.removeAll()
    blockConstraintOwnership.removeAll()
    // TUI takeover belongs to the Pane, not to a transient view tree;
    // preserve it while timeline projection rebuilds the presentation.
    subviews.forEach { $0.removeFromSuperview() }
    buildUI()
    needsLayout = true
    layoutSubtreeIfNeeded()
  }

  func buildUI() {
    let topChrome = makeTopChrome()
    let leftContext = makeLeftContextPanel()
    let pane = makeActiveTabSurface()
    let inspector = makeInspector()

    leftContext.isHidden = !isLeftContextVisible
    inspector.isHidden = !isInspectorVisible

    topChromeView = topChrome
    leftContextView = leftContext
    paneView = pane
    inspectorView = inspector

    [topChrome, leftContext, pane, inspector].forEach(addSubview)

    let leftWidth = isLeftContextVisible ? visual.metrics.leftContextWidth : 0
    let inspectorWidth = isInspectorVisible ? visual.metrics.inspectorWidth : 0
    let leftSeparator: CGFloat = isLeftContextVisible ? 1 : 0
    let rightSeparator: CGFloat = isInspectorVisible ? 1 : 0

    NSLayoutConstraint.activate([
      topChrome.leadingAnchor.constraint(equalTo: leadingAnchor),
      topChrome.trailingAnchor.constraint(equalTo: trailingAnchor),
      topChrome.topAnchor.constraint(equalTo: topAnchor),
      topChrome.heightAnchor.constraint(equalToConstant: visual.metrics.topChromeHeight),

      leftContext.leadingAnchor.constraint(equalTo: leadingAnchor),
      leftContext.topAnchor.constraint(equalTo: topChrome.bottomAnchor, constant: 1),
      leftContext.bottomAnchor.constraint(equalTo: bottomAnchor),
      leftContext.widthAnchor.constraint(equalToConstant: leftWidth),

      inspector.trailingAnchor.constraint(equalTo: trailingAnchor),
      inspector.topAnchor.constraint(equalTo: topChrome.bottomAnchor, constant: 1),
      inspector.bottomAnchor.constraint(equalTo: bottomAnchor),
      inspector.widthAnchor.constraint(equalToConstant: inspectorWidth),

      pane.leadingAnchor.constraint(equalTo: leftContext.trailingAnchor, constant: leftSeparator),
      pane.trailingAnchor.constraint(equalTo: inspector.leadingAnchor, constant: -rightSeparator),
      pane.topAnchor.constraint(equalTo: topChrome.bottomAnchor, constant: 1),
      pane.bottomAnchor.constraint(equalTo: bottomAnchor),
      pane.widthAnchor.constraint(greaterThanOrEqualToConstant: 520),
    ])
  }
  func debugLayoutContract() -> LayoutContract? {
    layoutSubtreeIfNeeded()
    guard let topChromeView,
      let leftContextView,
      let paneView,
      let inspectorView,
      let composerView
    else {
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

  func debugSetSidebarVisibility(left: Bool, inspector: Bool) {
    isLeftContextVisible = left
    isInspectorVisible = inspector
    rebuildUI()
  }

  func debugSetInspectorMode(_ mode: InspectorMode) {
    inspectorMode = mode
    rebuildUI()
  }

  func debugVisibleInspectorRows() -> [SeyalShellSnapshot.InspectorRow] {
    visibleInspectorRows()
  }

  func debugSnapshot() -> SeyalShellSnapshot {
    state.snapshot
  }

  static func smokeTest() -> Bool {
    let visual = SeyalThemeResolver.canonical(.dark)
    let shell = SeyalShellProductionFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
      visual: visual
    )
    shell.layoutSubtreeIfNeeded()
    guard let contract = shell.debugLayoutContract() else { return false }
    return shell.subviews.count == 4
      && abs(contract.topChrome.height - visual.metrics.topChromeHeight) < 1
      && abs(contract.leftContext.width - visual.metrics.leftContextWidth) < 1
      && abs(contract.inspector.width - visual.metrics.inspectorWidth) < 1
      && abs(contract.inspector.maxX - shell.bounds.maxX) < 1
      && abs(contract.pane.maxX - contract.inspector.minX + 1) < 1
      && contract.pane.width > 600
  }
}
