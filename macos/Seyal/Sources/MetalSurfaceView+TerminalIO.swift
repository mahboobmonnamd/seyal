import AppKit

@MainActor
extension MetalSurfaceView {
  @discardableResult
  func ensureTerminalBridgeConnected() -> Bool {
    guard bridge?.isConnected != true else { return true }
    guard !isDetachingRuntimeConnection,
      shouldAttachRuntime,
      bridge?.clientHandle == 0
    else { return false }
    if !bridgeRecoveryCoordinator.isActive,
      runtimeRecoveryState.stage != .blocked
    {
      bridgeRecoveryCoordinator.retry()
    }
    return false
  }

  @discardableResult
  func terminalSubmitCommittedText(_ text: String) -> Int32 {
    bridge?.submitCommittedText(text) ?? -10
  }

  @discardableResult
  func terminalSubmitPaste(_ text: String) -> Int32 {
    bridge?.submitPaste(text) ?? -10
  }

  @discardableResult
  func terminalSubmitHostSelection(
    action: UInt8,
    kind: UInt8 = 0,
    startCol: UInt16 = 0,
    startRow: UInt16 = 0,
    endCol: UInt16 = 0,
    endRow: UInt16 = 0
  ) -> Int32 {
    bridge?.submitHostSelection(
      action: action,
      kind: kind,
      startCol: startCol,
      startRow: startRow,
      endCol: endCol,
      endRow: endRow
    ) ?? -10
  }

  func terminalSubmitComposerCommand(_ text: String) -> Int32 {
    bridge?.submitComposerCommand(text) ?? -10
  }

  func terminalNextComposerRequestID() -> UInt64 {
    bridge?.nextComposerRequestID() ?? 0
  }

  func requestHistoryRange(startLine: UInt64, endLine: UInt64, blockID: UInt64) -> Int32 {
    bridge?.requestHistoryRange(startLine: startLine, endLine: endLine, blockID: blockID) ?? -10
  }

  func retainHistoryRange(_ range: NativeHistoryRange) {
    historyRanges[PaneBlockKey(paneID: paneID, blockID: range.blockID)] = range
  }

  func discardHistoryRequests(except blockIDs: Set<UInt64>) {
    bridge?.discardHistoryRequests(except: blockIDs)
  }

  @discardableResult
  func terminalSubmitKey(kind: UInt16, scalar: UInt32) -> Int32 {
    bridge?.submitKey(kind: kind, scalar: scalar) ?? -10
  }

  func terminalSupportsKeyV2() -> Bool {
    bridge?.supportsKeyV2() ?? false
  }

  /// Stop the current attachment so the existing recovery coordinator can
  /// establish a fresh connection. Used when V2 action IDs are exhausted.
  func terminalStopForProtocolRecovery() {
    bridge?.stop()
  }

  @discardableResult
  func terminalSubmitKeyV2(kind: UInt16, modifiers: UInt16, value: UInt32, event: UInt8, shiftedASCII: UInt32, actionID: UInt32) -> Int32 {
    bridge?.submitKeyV2(kind: kind, modifiers: modifiers, value: value, event: event, shiftedASCII: shiftedASCII, actionID: actionID) ?? -10
  }

  func terminalMouseCell(for event: NSEvent) -> (UInt16, UInt16)? {
    let point = convert(event.locationInWindow, from: nil)
    let cell = terminalPresentationCellSize()
    guard cell.width > 0, cell.height > 0 else { return nil }
    return bridge?.mouseCell(
      pixelX: Double(point.x),
      pixelYFromTop: Double(bounds.height - point.y),
      viewportWidth: Double(bounds.width),
      viewportHeight: Double(bounds.height),
      horizontalInsets: 0,
      verticalInsets: 0,
      cellWidth: Double(cell.width),
      cellHeight: Double(cell.height)
    )
  }

  @discardableResult
  func terminalSubmitMouse(
    kind: UInt8,
    button: UInt8,
    modifiers: UInt16,
    col: UInt16,
    row: UInt16,
    actionID: UInt32
  ) -> Int32 {
    bridge?.submitMouse(
      kind: kind,
      button: button,
      modifiers: modifiers,
      col: col,
      row: row,
      actionID: actionID
    ) ?? -10
  }

  @discardableResult
  func terminalProposeGeometry(
    viewportWidth: Double,
    viewportHeight: Double,
    horizontalInsets: Double,
    verticalInsets: Double,
    cellWidth: Double,
    cellHeight: Double,
    meaningfulLayoutEpoch: Bool
  ) -> Int32 {
    bridge?.proposeGeometry(
      viewportWidth: viewportWidth,
      viewportHeight: viewportHeight,
      horizontalInsets: horizontalInsets,
      verticalInsets: verticalInsets,
      cellWidth: cellWidth,
      cellHeight: cellHeight,
      meaningfulLayoutEpoch: meaningfulLayoutEpoch
    ) ?? -10
  }

  @discardableResult
  func terminalRetryResize() -> Int32 {
    bridge?.retryResize() ?? -10
  }

  func terminalInputFailureCode() -> Int32 {
    bridge?.inputFailureCode() ?? 4
  }

  func terminalResizeFailureCode() -> Int32 {
    bridge?.resizeFailureCode() ?? 201
  }

  func terminalCurrentFrame() -> SeyalPreparedFrame? {
    bridge?.currentFrame()
  }

  /// Republishes the latest committed frame after a presentation consumer
  /// installs its callback. The bridge may publish once during initialization
  /// before the surrounding Block body is attached.
  func publishCurrentTerminalFrame() {
    bridge?.publishCurrentFrame()
  }

  /// Installs the bounded Runtime history projection into this Pane's one
  /// Metal renderer. The callback is intentionally asynchronous at the
  /// bridge boundary but preparation itself remains main-thread confined with
  /// the rest of AppKit/Metal ownership.
  func renderHistoryRange(_ range: NativeHistoryRange, region: NativeTranscriptRegion? = nil) {
    retainHistoryRange(range)
    let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
    do {
      let rendererRegion: NativeTranscriptRegion
      if let region {
        // AppKit uses a bottom-left origin while the terminal shader
        // consumes top-left pixel coordinates.
        let pixelClip = NSRect(
          x: region.clip.minX * scale,
          y: (bounds.height - region.clip.maxY) * scale,
          width: region.clip.width * scale,
          height: region.clip.height * scale
        )
        rendererRegion = NativeTranscriptRegion(
          id: region.id,
          origin: pixelClip.origin,
          clip: pixelClip
        )
      } else {
        rendererRegion = NativeTranscriptRegion(
          id: range.blockID,
          origin: .zero,
          clip: NSRect(
            x: 0,
            y: 0,
            width: bounds.width * scale,
            height: bounds.height * scale
          )
        )
      }
      try renderer.update(
        historyRange: range,
        region: rendererRegion,
        backingScale: scale
      )
      if shouldRender {
        renderer.requestPresent()
        beginPresentationAttemptSeries()
        armMetalDisplayLink()
      }
      refreshRecoveryAccessibilityValue()
    } catch {
      lastRenderError = error
    }
  }

  func applyRendererPresentation(_ plan: RendererPresentationPlan) {
    renderer.setPresentationPlan(plan)
    layer?.isOpaque = plan.drawsFullGridBackground
    if let metalLayer = layer as? CAMetalLayer {
      metalLayer.isOpaque = plan.drawsFullGridBackground
      // Flow composites glyphs over AppKit Block chrome. The compositor must
      // be able to sample alpha; framebuffer-only drawables skip that path.
      metalLayer.framebufferOnly = plan.drawsFullGridBackground
    }
  }

  override var isOpaque: Bool {
    inspectRendererPresentation().drawsFullGridBackground
  }

  override func hitTest(_ point: NSPoint) -> NSView? {
    // Flow paints terminal pixels over Block bodies. Chrome owns scrolling
    // and composer hit-testing, so the compositor must not swallow events.
    if inspectRendererPresentation().drawsLiveGrid {
      return super.hitTest(point)
    }
    return nil
  }

  func inspectRendererPresentation() -> RendererPresentationInspection {
    renderer.inspectPresentation()
  }

  func inspectFlowPaint(sampleGPU: Bool = false) -> FlowPaintInspection {
    guard sampleGPU else {
      return renderer.inspectFlowPaint()
    }
    let size = convertToBacking(bounds).size
    let width = max(1, Int(size.width.rounded()))
    let height = max(1, Int(size.height.rounded()))
    guard let texture = renderer.renderOffscreenAndWait(width: width, height: height) else {
      return renderer.inspectFlowPaint()
    }
    return renderer.inspectFlowPaint(from: texture)
  }

  func setTranscriptFrame(_ frame: NativeTranscriptFrame) {
    guard frame.isValid,
      frame.surfaceIdentity == nil || frame.surfaceIdentity == ObjectIdentifier(self)
    else {
      return
    }
    let regionIDs = Set(frame.regionIDs)
    historyRanges = historyRanges.filter { regionIDs.contains($0.key.blockID) }
    renderer.removeHistoryRegions(except: regionIDs)
    renderer.setHistoryRegionOrder(frame.regionIDs)
    // Body intrinsic growth moves every following Block. Re-encode all
    // retained canonical ranges against this complete frame so no region
    // retains its previous clip or origin.
    for region in frame.regions {
      guard let range = historyRanges[PaneBlockKey(paneID: paneID, blockID: region.id)] else {
        continue
      }
      renderHistoryRange(range, region: region)
    }
  }

  func removeTranscriptRegions(except ids: Set<UInt64>) {
    historyRanges = historyRanges.filter { ids.contains($0.key.blockID) }
    renderer.removeHistoryRegions(except: ids)
  }

  var terminalExecutionIdentity: String? {
    guard terminalBridgeIsConnected, let bridge else { return nil }
    return Self.identityString((
      low: bridge.lastRecoveryResult.executionIDLow,
      high: bridge.lastRecoveryResult.executionIDHigh
    ))
  }

  var terminalRuntimeIdentity: String? {
    guard terminalBridgeIsConnected, let bridge else { return nil }
    return Self.identityString(bridge.runtimeIdentityWords)
  }

  var terminalAttachmentIdentity: String? {
    guard terminalBridgeIsConnected, let bridge else { return nil }
    return Self.identityString(bridge.attachmentIdentityWords)
  }

  /// Makes the authoritative recovery state observable to VoiceOver and to
  /// native acceptance automation without introducing a second terminal
  /// model or changing the Runtime protocol.
  func refreshRecoveryAccessibilityValue() {
    let connection = terminalBridgeIsConnected ? "usable" : "disconnected"
    let runtime = terminalRuntimeIdentity ?? "none"
    let execution = terminalExecutionIdentity ?? "none"
    let attachment = terminalAttachmentIdentity ?? "none"
    let alternate = lastAlternateScreen == true ? "true" : "false"
    let flowPaint = renderer.inspectFlowPaint().accessibilityToken
    setAccessibilityValue(
      "process=\(ProcessInfo.processInfo.processIdentifier) connection=\(connection) "
        + "runtime=\(runtime) execution=\(execution) "
        + "attachment=\(attachment) alternate-screen=\(alternate) "
        + "flow-paint=\(flowPaint)"
    )
  }

  private static func identityString(
    _ words: (low: UInt64, high: UInt64)
  ) -> String? {
    guard words.low != 0 || words.high != 0 else { return nil }
    return String(format: "%016llx%016llx", words.high, words.low)
  }

  /// Logical cell metrics come from the permanent renderer's font/atlas metric
  /// source. Resize code must not independently remeasure fonts.
  func terminalLogicalCellSize() -> CGSize {
    let pixels = renderer.cellPixelSize(backingScale: 1)
    return CGSize(width: CGFloat(pixels.width), height: CGFloat(pixels.height))
  }

  /// Candidate-window anchoring needs logical points for the current screen.
  func terminalPresentationCellSize() -> CGSize {
    let scale = max(window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1, 1)
    let pixels = renderer.cellPixelSize(backingScale: scale)
    return CGSize(
      width: CGFloat(pixels.width) / scale,
      height: CGFloat(pixels.height) / scale
    )
  }

}
