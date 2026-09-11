import AppKit

/// Timeline chrome is laid over the one input surface. Empty areas must remain
/// hit-test transparent so keyboard/mouse events continue to reach the
/// Runtime-backed terminal; future interactive Block controls can still be
/// returned by the normal descendant hit-test.
@MainActor
final class TranscriptBlockStackView: NSStackView {
  override func hitTest(_ point: NSPoint) -> NSView? {
    guard let hit = super.hitTest(point) else { return nil }
    if hit is NSButton { return hit }
    if let field = hit as? NSTextField, field.isEditable || field.isSelectable {
      return hit
    }
    return nil
  }
}

/// Holds Block chrome above the clip-hosted Metal surface. Empty document
/// hits must fall through so the one terminal surface keeps first-responder
/// and mouse delivery.
@MainActor
final class TranscriptDocumentView: NSView {
  override func hitTest(_ point: NSPoint) -> NSView? {
    let hit = super.hitTest(point)
    return hit === self ? nil : hit
  }
}

@MainActor
final class CommandBlockBodyView: NSView {
  private weak var surface: InteractiveMetalSurfaceView?
  private(set) var historyRange: NativeHistoryRange?
  var onContentSizeChanged: (() -> Void)?
  private(set) var blockID: UInt64 = 0
  /// The renderer's canonical row projection is line-oriented. Keeping the
  /// row height here gives Auto Layout a real document extent immediately,
  /// then expands it when the bounded range arrives asynchronously.
  let rowHeight: CGFloat = 18
  private var projectedRowCount = 1

  init(surface: InteractiveMetalSurfaceView? = nil, installSurface: Bool = true) {
    self.surface = surface
    super.init(frame: .zero)
    translatesAutoresizingMaskIntoConstraints = false
    setContentHuggingPriority(.defaultLow, for: .vertical)
    setContentCompressionResistancePriority(.required, for: .vertical)
    if let surface {
      surface.isHidden = false
      addSubview(surface)
      NSLayoutConstraint.activate([
        surface.leadingAnchor.constraint(equalTo: leadingAnchor),
        surface.trailingAnchor.constraint(equalTo: trailingAnchor),
        surface.topAnchor.constraint(equalTo: topAnchor),
        surface.bottomAnchor.constraint(equalTo: bottomAnchor),
      ])
    } else {
      // Historical output is rendered by the single Pane-owned Metal
      // surface installed by the transcript. This body deliberately has
      // no AppKit text fallback: copying terminal cells into a text view
      // would create a second, stale terminal authority. It still has a
      // non-zero intrinsic extent so a Block remains a visible document
      // region while its canonical rows are being requested.
    }
  }

  override var intrinsicContentSize: NSSize {
    NSSize(width: NSView.noIntrinsicMetric, height: CGFloat(projectedRowCount) * rowHeight)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("CommandBlockBodyView is programmatic") }

  func setTUI(_ active: Bool) {
    // A TUI changes chrome/layout around the same canonical Metal surface;
    // it never swaps in an AppKit text transcript.
    surface?.isHidden = false
  }

  func attach(to blockID: UInt64) {
    guard blockID != 0 else { return }
    self.blockID = blockID
  }

  /// Retains the bounded Runtime projection for the owning Block. The
  /// projection remains structured cells (including style) and is consumed
  /// by the Pane surface; no AppKit text copy is created here.
  func setHistoryRange(_ range: NativeHistoryRange) {
    guard range.blockID != 0, range.requestID != 0,
      blockID == 0 || blockID == range.blockID,
      historyRange.map({ range.revision >= $0.revision }) ?? true
    else { return }
    historyRange = range
    projectedRowCount = max(1, range.rows.count)
    invalidateIntrinsicContentSize()
    needsLayout = true
    onContentSizeChanged?()
    setAccessibilityValue("history-lines:\(range.rows.count)")
  }
}

/// The one scroll/document owner for a Pane's Flow transcript. The document
/// contains the Warp-style Block cards while the one bridge-backed Metal
/// surface remains a sibling underneath them, preserving one PTY/VT authority
/// and one input surface across timeline updates.
@MainActor
final class PaneTranscriptView: NSScrollView {
  private let paneID: String
  let transcriptDocument = TranscriptDocumentView(frame: .zero)
  let terminalSurface: InteractiveMetalSurfaceView
  private var blockBodies: [PaneBlockKey: NSView] = [:]
  private var blockOrder: [PaneBlockKey] = []
  private var frameRevision: UInt64 = 0
  private var isRefreshingFromClipBounds = false
  var onBlockBodySizeChanged: (() -> Void)?

  init(
    paneID: String = "unbound",
    surface: InteractiveMetalSurfaceView? = nil,
    installSurface: Bool = true,
    executionIdentity: String? = nil,
    allowsImplicitExecutionBootstrap: Bool = true,
    visual: SeyalResolvedVisualConfiguration
  ) {
    self.paneID = paneID
    terminalSurface = surface ?? InteractiveMetalSurfaceView(
      frame: .zero,
      paneID: paneID,
      executionIdentity: executionIdentity,
      allowsImplicitExecutionBootstrap: allowsImplicitExecutionBootstrap,
      terminalFont: visual.terminalFont
    )
    // Flow keyboard belongs to the composer. Set this before layout/recovery
    // so renderer-ready restore cannot steal first responder (that is what
    // broke Enter after Metal moved to a viewport sibling).
    terminalSurface.claimsFirstResponderOnClick = false
    super.init(frame: .zero)
    translatesAutoresizingMaskIntoConstraints = false
    drawsBackground = false
    hasVerticalScroller = true
    hasHorizontalScroller = false
    autohidesScrollers = true
    borderType = .noBorder
    // CAMetalLayer inside a scrolling document is copied by NSClipView and
    // bilinear-stretched into smear. Keep Metal on the clip viewport instead.
    contentView.copiesOnScroll = false
    contentView.postsBoundsChangedNotifications = true
    contentView.wantsLayer = true
    contentView.drawsBackground = false
    contentView.layer?.backgroundColor = visual.colors.cg(.canvas)
    transcriptDocument.translatesAutoresizingMaskIntoConstraints = false
    transcriptDocument.wantsLayer = true
    transcriptDocument.layer?.backgroundColor = .clear
    documentView = transcriptDocument

    // NSScrollView does not infer a document width from edge constraints on
    // subviews. Pin the document to the clip view's viewport bounds so Block
    // chrome and history clips share the final visible width. The explicit
    // layout pass below avoids the transient zero/narrow content bounds seen
    // during startup.
    transcriptDocument.widthAnchor.constraint(equalTo: contentView.widthAnchor).isActive = true

    NotificationCenter.default.addObserver(
      self,
      selector: #selector(clipViewBoundsDidChange),
      name: NSView.boundsDidChangeNotification,
      object: contentView
    )

    if installSurface {
      // Viewport-sized overlay on the scroll view, not inside NSClipView.
      // Extra clip-view subviews drop out of the key-view loop, so Enter and
      // paste never reach the composer or the terminal. Matching the clip
      // frame keeps the drawable at the visible size (no document-tall smear).
      terminalSurface.translatesAutoresizingMaskIntoConstraints = true
      terminalSurface.autoresizingMask = []
      addSubview(terminalSurface, positioned: .below, relativeTo: contentView)
      syncMetalSurfaceFrame()
    }
  }

  deinit {
    NotificationCenter.default.removeObserver(self)
  }

  @objc private func clipViewBoundsDidChange() {
    guard !isRefreshingFromClipBounds else { return }
    isRefreshingFromClipBounds = true
    defer { isRefreshingFromClipBounds = false }
    syncMetalSurfaceFrame()
    refreshTranscriptFrame()
  }

  private func syncMetalSurfaceFrame() {
    guard terminalSurface.superview === self else { return }
    terminalSurface.frame = contentView.frame
  }

  override func hitTest(_ point: NSPoint) -> NSView? {
    let hit = super.hitTest(point)
    if hit === contentView || hit === transcriptDocument,
      terminalSurface.superview === self
    {
      return terminalSurface
    }
    return hit
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) { fatalError("PaneTranscriptView is programmatic") }

  override func layout() {
    super.layout()
    // AppKit may lay out the scroll view and its document in separate passes.
    // Complete both passes before the interactive surface samples its viewport
    // for Runtime geometry, so a startup narrow width cannot become sticky.
    syncMetalSurfaceFrame()
    transcriptDocument.layoutSubtreeIfNeeded()
    terminalSurface.needsLayout = true
    terminalSurface.layoutSubtreeIfNeeded()
  }

  func applyVisual(_ visual: SeyalResolvedVisualConfiguration) {
    contentView.layer?.backgroundColor = visual.colors.cg(.canvas)
  }

  func installBlockStack(_ stack: NSStackView) {
    stack.translatesAutoresizingMaskIntoConstraints = false
    transcriptDocument.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: transcriptDocument.leadingAnchor, constant: 8),
      stack.trailingAnchor.constraint(equalTo: transcriptDocument.trailingAnchor, constant: -8),
      stack.topAnchor.constraint(equalTo: transcriptDocument.topAnchor, constant: 8),
      stack.bottomAnchor.constraint(equalTo: transcriptDocument.bottomAnchor, constant: -8),
    ])
  }

  /// Registers every Block body against this Pane's one persistent surface.
  /// Registration order is the authoritative timeline order; geometry is
  /// sampled only when a complete transcript frame is requested.
  func registerBlockBody(_ body: NSView, blockID: UInt64) {
    guard blockID != 0 else { return }
    let key = PaneBlockKey(paneID: paneID, blockID: blockID)
    if let body = body as? CommandBlockBodyView {
      body.attach(to: blockID)
      body.onContentSizeChanged = { [weak self] in
        self?.refreshTranscriptFrame()
        self?.onBlockBodySizeChanged?()
      }
    }
    if blockBodies[key] == nil { blockOrder.append(key) }
    blockBodies[key] = body
    frameRevision &+= 1
  }

  func unregisterMissingBlockBodies(_ ids: Set<UInt64>) {
    blockOrder.removeAll { key in
      guard ids.contains(key.blockID) else {
        blockBodies.removeValue(forKey: key)
        return true
      }
      return false
    }
    frameRevision &+= 1
  }

  /// Replaces the authoritative timeline order without replacing any body.
  /// Reordering is a layout operation; identity and canonical pixels stay
  /// attached to their existing Block views.
  func replaceBlockOrder(_ ids: [UInt64]) {
    guard Set(ids).count == ids.count, ids.allSatisfy({ $0 != 0 }) else { return }
    blockOrder = ids.map { PaneBlockKey(paneID: paneID, blockID: $0) }
      .filter { blockBodies[$0] != nil }
    frameRevision &+= 1
  }

  func region(for blockID: UInt64) -> NativeTranscriptRegion? {
    let key = PaneBlockKey(paneID: paneID, blockID: blockID)
    guard let body = blockBodies[key] else { return nil }
    let clip = body.convert(body.bounds, to: terminalSurface)
    return NativeTranscriptRegion(id: blockID, origin: clip.origin, clip: clip)
  }

  /// Returns an atomic geometry snapshot for all currently registered Block
  /// bodies. No caller may submit a subset of these regions.
  func transcriptFrame() -> NativeTranscriptFrame {
    layoutSubtreeIfNeeded()
    let regions = blockOrder.compactMap { region(for: $0.blockID) }
    return NativeTranscriptFrame(
      revision: frameRevision,
      regions: regions,
      surfaceIdentity: ObjectIdentifier(terminalSurface)
    )
  }

  /// Re-layouts the whole ordered Block document after one history response
  /// changes intrinsic height. Every region is recomputed in one snapshot so
  /// Metal never receives a partially updated order or stale clip.
  func refreshTranscriptFrame() {
    needsLayout = true
    layoutSubtreeIfNeeded()
    let frame = transcriptFrame()
    terminalSurface.setTranscriptFrame(frame)
  }
}
