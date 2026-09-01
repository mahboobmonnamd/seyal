import AppKit
import Metal
@preconcurrency import QuartzCore

enum RuntimeRecoveryStage: UInt8, Equatable {
  case disconnected = 0
  case discovering = 1
  case startingRuntime = 2
  case waitingForController = 3
  case reconstructing = 4
  case restoringInteraction = 5
  case usable = 6
  case exhausted = 7
  case blocked = 8
}

struct RuntimeRecoveryState: Equatable {
  private(set) var stage: RuntimeRecoveryStage = .disconnected
  private(set) var generation: UInt64 = 0

  mutating func begin() {
    generation &+= 1
    stage = .discovering
  }

  mutating func transition(to next: RuntimeRecoveryStage) {
    stage = next
  }

  mutating func cancel() {
    generation &+= 1
    stage = .disconnected
  }

  mutating func retry() {
    begin()
  }
}

struct PresentationRetryBudget: Equatable {
  private static let delays: [TimeInterval] = [1.0 / 60.0, 0.05, 0.20, 0.75]
  static let maximumAutomaticRetries = 4
  private var nextAttempt = 0

  mutating func reset() {
    nextAttempt = 0
  }

  mutating func claimNextDelay() -> TimeInterval? {
    guard nextAttempt < Self.delays.count else { return nil }
    defer { nextAttempt += 1 }
    return Self.delays[nextAttempt]
  }

  var exhausted: Bool {
    nextAttempt >= Self.delays.count
  }
}

struct PresentationOpportunityState: Equatable {
  private(set) var pending = false
  private(set) var armed = false

  mutating func request() {
    pending = true
  }

  mutating func armIfNeeded() -> Bool {
    guard pending, !armed else { return false }
    armed = true
    return true
  }

  mutating func consumeOpportunity() -> Bool {
    guard armed, pending else { return false }
    armed = false
    return true
  }

  mutating func markPresented() {
    pending = false
    armed = false
  }

  mutating func markFailed() {
    pending = true
    armed = false
  }

  mutating func cancel() {
    pending = false
    armed = false
  }
}

struct PresentationRecoveryState: Equatable {
  private(set) var opportunity = PresentationOpportunityState()
  private(set) var retryBudget = PresentationRetryBudget()
  private(set) var exhausted = false

  var pending: Bool { opportunity.pending }
  var armed: Bool { opportunity.armed }

  mutating func request() {
    guard !exhausted else { return }
    opportunity.request()
  }

  mutating func armIfNeeded() -> Bool {
    guard !exhausted else { return false }
    return opportunity.armIfNeeded()
  }

  mutating func consumeOpportunity() -> Bool {
    opportunity.consumeOpportunity()
  }

  mutating func recordSubmissionFailure() -> TimeInterval? {
    opportunity.markFailed()
    guard let delay = retryBudget.claimNextDelay() else {
      exhausted = true
      opportunity.cancel()
      return nil
    }
    return delay
  }

  mutating func recordSubmissionSuccess() {
    opportunity.markPresented()
    retryBudget.reset()
    exhausted = false
  }

  mutating func cancel() {
    opportunity.cancel()
    retryBudget.reset()
    exhausted = false
  }

  mutating func cancelPending() {
    opportunity.cancel()
  }

  mutating func resetForLifecycleRecovery() {
    cancel()
  }
}

struct PreparationRecoveryState: Equatable {
  private(set) var retryBudget = PresentationRetryBudget()
  private(set) var exhausted = false

  var canAttemptPreparation: Bool { !exhausted }

  mutating func recordFailure() -> TimeInterval? {
    guard !exhausted else { return nil }
    guard let delay = retryBudget.claimNextDelay() else {
      exhausted = true
      return nil
    }
    return delay
  }

  mutating func recordSuccess() {
    retryBudget.reset()
    exhausted = false
  }

  mutating func resetForLifecycleRecovery() {
    recordSuccess()
  }
}

/// Owns the run-loop registration independently of the view's actor-isolated
/// lifetime. Releasing a surface therefore also invalidates its display link.
final class MetalDisplayLinkLease {
  let link: CAMetalDisplayLink

  init(layer: CAMetalLayer) {
    link = CAMetalDisplayLink(metalLayer: layer)
  }

  deinit {
    link.delegate = nil
    link.invalidate()
  }
}

@MainActor
class MetalSurfaceView: NSView, @MainActor CAMetalDisplayLinkDelegate {
  let paneID: String
  let requestedExecutionIdentity: String?
  let allowsImplicitExecutionBootstrap: Bool
  private let metalDevice: any MTLDevice
  private let renderer: MetalTerminalRenderer
  private var bridge: RustDisplayBridge?
  private lazy var bridgeRecoveryCoordinator = RuntimeLifecycleRecoveryCoordinator(
    clock: { CACurrentMediaTime() },
    scheduler: { delay, operation in
      let timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
        MainActor.assumeIsolated { operation() }
      }
      return { timer.invalidate() }
    },
    launcher: { [weak self] in self?.bridge?.launchBundledRuntime() },
    attempt: { [weak self] in self?.attemptBridgeRecovery() ?? .blocked }
  )
  var runtimeRecoveryState: RuntimeRecoveryState { bridgeRecoveryCoordinator.state }
  private var forceNextFrame = false
  private var hasPreparedState = false
  private var presentationState = PresentationRecoveryState()
  private var presentationRetryScheduled = false
  private var presentationRetryTimer: Timer?
  private var presentationRetryGeneration: UInt64 = 0
  private var renderable = false
  private var metalDisplayLinkLease: MetalDisplayLinkLease?
  private var preparationRetryTimer: Timer?
  private var preparationRetryGeneration: UInt64 = 0
  private var preparationRetryScheduled = false
  private var preparationState = PreparationRecoveryState()
  private var lastAlternateScreen: Bool?
  private(set) var lastBridgeError: Int32?
  private(set) var lastRenderError: Error?
  private var historyRanges: [PaneBlockKey: NativeHistoryRange] = [:]

  override convenience init(frame frameRect: NSRect) {
    self.init(frame: frameRect, paneID: "unbound")
  }

  init(
    frame frameRect: NSRect,
    paneID: String,
    executionIdentity: String? = nil,
    allowsImplicitExecutionBootstrap: Bool = true
  ) {
    self.paneID = paneID
    self.requestedExecutionIdentity = executionIdentity
    self.allowsImplicitExecutionBootstrap = allowsImplicitExecutionBootstrap
    guard let device = MTLCreateSystemDefaultDevice() else {
      fatalError("Seyal requires a Metal-capable macOS device")
    }
    let renderer: MetalTerminalRenderer
    do {
      renderer = try MetalTerminalRenderer(device: device)
    } catch {
      fatalError("Seyal permanent Metal renderer initialization failed: \(error)")
    }

    metalDevice = device
    self.renderer = renderer
    super.init(frame: frameRect)
    wantsLayer = true

    guard let metalLayer = layer as? CAMetalLayer else {
      fatalError("MetalSurfaceView backing layer must be CAMetalLayer")
    }
    metalLayer.device = device
    metalLayer.pixelFormat = .bgra8Unorm
    metalLayer.framebufferOnly = true
    metalLayer.maximumDrawableCount = 2
    metalLayer.presentsWithTransaction = false
    updateDrawableSize()

    // No dedicated GPU surface resources are retained before the view is
    // actually visible. Candidate-D state may still advance independently.
    renderer.setVisible(false)
    renderer.onNeedsCurrentFrame = { [weak self] in
      self?.bridge?.publishCurrentFrame()
    }
    renderer.onPersistentDisplayFailure = { [weak self] error in
      self?.lastRenderError = error
    }

    let bridge = RustDisplayBridge(
      onFrame: { [weak self] frame in
        self?.consumeBridgeFrame(frame)
      },
      onError: { [weak self] code in
        self?.lastBridgeError = code
        DispatchQueue.main.async { [weak self] in
          self?.terminalBridgeDidFail(code)
        }
      },
      onStatusChanged: { [weak self] in
        DispatchQueue.main.async { [weak self] in
          self?.terminalBridgeStatusDidChange()
        }
      },
      onTimeline: { [weak self] records in
        self?.onTimelineChanged?(records)
      },
      onHistory: { [weak self] range in
        self?.onHistoryRangeChanged?(range)
      },
      onComposerResult: { [weak self] result in
        self?.onComposerResultChanged?(result)
      },
      paneID: paneID,
      executionIdentity: executionIdentity,
      allowsImplicitExecutionBootstrap: allowsImplicitExecutionBootstrap
    )
    self.bridge = bridge
    _ = bridge.start()
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) {
    fatalError("Seyal uses a programmatic AppKit/Metal surface")
  }

  override func makeBackingLayer() -> CALayer {
    CAMetalLayer()
  }

  /// Narrow subclass hooks for Pass 7 presentation-only failure/focus state.
  /// They never transfer PTY, VT, grid or renderer authority into AppKit.
  func terminalBridgeDidFail(_ code: Int32) {
    _ = code
  }

  func terminalBridgeStatusDidChange() {
    refreshRecoveryAccessibilityValue()
    guard bridge?.isConnected != true else { return }
    // History/composer/display correlations are disposable connection state;
    // logical pane and Block identity remain owned by Runtime and are not
    // cleared here.
    historyRanges.removeAll(keepingCapacity: false)
    invalidatePreparedPresentation()
  }

  /// Presentation-only notification. Runtime/Metal remains authoritative;
  /// AppKit uses this to switch the surrounding Pane chrome.
  var onAlternateScreenChanged: ((Bool) -> Void)?
  var onFrameChanged: ((NativePreparedFrame) -> Void)?
  var onTimelineChanged: (([NativeBlockRecord]) -> Void)?
  var onHistoryRangeChanged: ((NativeHistoryRange) -> Void)?
  var onComposerResultChanged: ((NativeComposerResult) -> Void)?

  var terminalBridgeIsConnected: Bool {
    bridge?.isConnected == true
  }

  /// Reconnects the pane's existing PTY bridge at the command boundary.
  /// Block timeline updates may recreate presentation views, but a command
  /// must never be rejected merely because that view has just re-entered the
  /// window hierarchy.
  @discardableResult
  func ensureTerminalBridgeConnected() -> Bool {
    guard bridge?.isConnected != true else { return true }
    return bridge?.start() == true
  }

  @discardableResult
  func terminalSubmitCommittedText(_ text: String) -> Int32 {
    bridge?.submitCommittedText(text) ?? -10
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

  func discardHistoryRequests(except blockIDs: Set<UInt64>) {
    bridge?.discardHistoryRequests(except: blockIDs)
  }

  @discardableResult
  func terminalSubmitKey(kind: UInt16, scalar: UInt32) -> Int32 {
    bridge?.submitKey(kind: kind, scalar: scalar) ?? -10
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
    let key = PaneBlockKey(paneID: paneID, blockID: range.blockID)
    historyRanges[key] = range
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
    } catch {
      lastRenderError = error
    }
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
    setAccessibilityValue(
      "process=\(ProcessInfo.processInfo.processIdentifier) connection=\(connection) "
        + "runtime=\(runtime) execution=\(execution) "
        + "attachment=\(attachment) alternate-screen=\(alternate)"
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

  override func layout() {
    super.layout()
    updateDrawableSize()
    guard shouldRender,
      hasPreparedState,
      renderer.persistentDisplayFailure == nil,
      !presentationState.exhausted
    else { return }
    renderer.requestPresent()
    beginPresentationAttemptSeries()
    armMetalDisplayLink()
  }

  override func viewDidChangeBackingProperties() {
    super.viewDidChangeBackingProperties()
    updateDrawableSize()
    forceNextFrame = true
    if shouldRender {
      bridge?.publishCurrentFrame()
    }
  }

  override func viewDidHide() {
    super.viewDidHide()
    updateVisibility()
  }

  override func viewDidUnhide() {
    super.viewDidUnhide()
    updateVisibility()
  }

  override func viewWillMove(toWindow newWindow: NSWindow?) {
    if let window {
      NotificationCenter.default.removeObserver(
        self,
        name: NSWindow.didChangeOcclusionStateNotification,
        object: window
      )
    }
    if newWindow == nil {
      renderer.setVisible(false)
      invalidatePreparedPresentation()
      invalidateMetalDisplayLink()
      cancelBridgeReconnect()
      bridge?.stop()
    }
    super.viewWillMove(toWindow: newWindow)
  }

  override func viewDidMoveToWindow() {
    super.viewDidMoveToWindow()
    if let window {
      NotificationCenter.default.addObserver(
        self,
        selector: #selector(windowOcclusionChanged),
        name: NSWindow.didChangeOcclusionStateNotification,
        object: window
      )
    }
    updateDrawableSize()
    updateVisibility()
  }

  @objc private func windowOcclusionChanged() {
    updateVisibility()
  }

  private var shouldRender: Bool {
    guard let window else { return false }
    return !window.isMiniaturized
      && window.occlusionState.contains(.visible)
      && !isHiddenOrHasHiddenAncestor
  }

  private func attemptBridgeRecovery() -> RuntimeRecoveryAttemptOutcome {
    guard shouldRender, let bridge, !bridge.isConnected else {
      return bridge?.isConnected == true ? .connected : .blocked
    }
    guard !bridge.start() else { return .connected }
    let result = bridge.lastRecoveryResult
    if result.failureClass == 1, result.retryable {
      return .endpointMissing
    }
    if result.failureClass == 3, result.retryable {
      return .controllerBusy
    }
    return result.retryable ? .retryable : .blocked
  }

  private func cancelBridgeReconnect() {
    bridgeRecoveryCoordinator.cancel()
  }

  /// Explicit user retry starts a new bounded foreground recovery episode.
  /// Automatic exhaustion never invokes this method recursively.
  @discardableResult
  func retryRuntimeConnection() -> Bool {
    guard shouldRender, bridge?.isConnected != true else { return true }
    bridgeRecoveryCoordinator.retry()
    return bridge?.isConnected == true
  }

  private func updateVisibility() {
    let renderable = shouldRender
    let becameRenderable = renderable && !self.renderable
    self.renderable = renderable
    if renderable {
      if let metalLayer = layer as? CAMetalLayer {
        installMetalDisplayLink(on: metalLayer)
      }
      forceNextFrame = true
      if bridge?.isConnected == false {
        if !bridgeRecoveryCoordinator.isActive,
          runtimeRecoveryState.stage != .exhausted,
          runtimeRecoveryState.stage != .blocked
        {
          bridgeRecoveryCoordinator.beginEpisode()
        }
      }
    } else {
      invalidateMetalDisplayLink()
      invalidatePreparedPresentation()
    }

    renderer.setVisible(renderable)
    if renderable {
      // Showing is the explicit recovery boundary for an exhausted GPU
      // completion failure series. Reconstruct from the latest committed
      // Candidate-D state; never request PTY-byte replay.
      if becameRenderable {
        presentationState.resetForLifecycleRecovery()
        lastRenderError = nil
      } else if renderer.persistentDisplayFailure == nil,
        !presentationState.exhausted
      {
        lastRenderError = nil
      }
      bridge?.publishCurrentFrame()
      if hasPreparedState, !presentationState.exhausted {
        renderer.requestPresent()
        beginPresentationAttemptSeries()
        armMetalDisplayLink()
      }
    }
  }

  private func consumeBridgeFrame(_ bridgeFrame: SeyalPreparedFrame) {
    guard !presentationState.exhausted,
      preparationState.canAttemptPreparation
    else {
      forceNextFrame = true
      return
    }
    guard let frame = NativePreparedFrame(bridgeFrame: bridgeFrame) else {
      return
    }
    onFrameChanged?(frame)
    if lastAlternateScreen != frame.alternateScreen {
      lastAlternateScreen = frame.alternateScreen
      onAlternateScreenChanged?(frame.alternateScreen)
    }
    refreshRecoveryAccessibilityValue()
    let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
    do {
      let result = try renderer.update(
        frame: frame,
        backingScale: scale,
        forceFullRebuild: forceNextFrame
      )
      if result == .updated {
        forceNextFrame = false
        hasPreparedState = true
        bridgeRecoveryCoordinator.transition(to: .restoringInteraction)
        // Candidate-D can continue advancing while an exhausted GPU
        // display failure is latched. A successful CPU preparation must
        // not erase that asynchronous display diagnostic.
        if renderer.persistentDisplayFailure == nil,
          !presentationState.exhausted
        {
          lastRenderError = nil
        }
        resetPreparationRecovery()
        if shouldRender,
          bridge?.isConnected == true,
          hasPreparedState,
          !presentationState.exhausted
        {
          bridgeRecoveryCoordinator.transition(to: .usable)
        }
        if shouldRender,
          renderer.persistentDisplayFailure == nil,
          !presentationState.exhausted
        {
          beginPresentationAttemptSeries()
          armMetalDisplayLink()
        }
      }
    } catch {
      lastRenderError = error
      // Renderer preparation is incremental for damage efficiency.  A
      // failed replacement must not leave a partially updated live
      // buffer eligible for a later present.
      hasPreparedState = false
      // Keep the preparation recovery series alive across failures. The
      // lifecycle invalidation path resets it; resetting here would make
      // a persistent resource failure retry forever at the first delay.
      cancelPresentationOpportunity()
      renderer.invalidatePreparedState()
      forceNextFrame = true
      guard let delay = preparationState.recordFailure() else {
        lastRenderError = MetalTerminalRendererError.preparationFailuresExhausted
        return
      }
      schedulePreparationRetry(after: delay)
    }
  }

  private func schedulePreparationRetry(after delay: TimeInterval) {
    guard shouldRender,
      !hasPreparedState,
      !preparationRetryScheduled,
      preparationState.canAttemptPreparation
    else {
      return
    }

    preparationRetryScheduled = true
    let generation = preparationRetryGeneration
    preparationRetryTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) {
      [weak self] _ in
      Task { @MainActor [weak self] in
        self?.runPreparationRetry(generation: generation)
      }
    }
  }

  private func runPreparationRetry(generation: UInt64) {
    guard generation == preparationRetryGeneration else { return }
    preparationRetryTimer = nil
    preparationRetryScheduled = false
    guard shouldRender, !hasPreparedState else { return }
    // Publish only after the current update call has unwound. This keeps
    // retry recovery asynchronous and avoids re-entering renderer.update.
    bridge?.publishCurrentFrame()
  }

  private func resetPreparationRetries() {
    preparationRetryTimer?.invalidate()
    preparationRetryTimer = nil
    preparationRetryGeneration &+= 1
    preparationRetryScheduled = false
  }

  private func resetPreparationRecovery() {
    resetPreparationRetries()
    preparationState.resetForLifecycleRecovery()
  }

  private func installMetalDisplayLink(on metalLayer: CAMetalLayer) {
    guard metalDisplayLinkLease == nil else { return }
    let lease = MetalDisplayLinkLease(layer: metalLayer)
    let link = lease.link
    link.delegate = self
    link.isPaused = true
    link.add(to: .main, forMode: .common)
    metalDisplayLinkLease = lease
  }

  private func invalidateMetalDisplayLink() {
    metalDisplayLinkLease = nil
  }

  private func armMetalDisplayLink() {
    guard shouldRender,
      hasPreparedState,
      presentationState.pending,
      !presentationState.exhausted,
      renderer.hasPresentablePreparedState,
      !renderer.hasFrameInFlight
    else {
      return
    }
    if presentationState.armIfNeeded() {
      metalDisplayLinkLease?.link.isPaused = false
    }
  }

  func metalDisplayLink(
    _ link: CAMetalDisplayLink,
    needsUpdate update: CAMetalDisplayLink.Update
  ) {
    // The callback may already be queued when the view is detached or the
    // display link is replaced. Never let an old link present into a new
    // surface lifecycle.
    guard metalDisplayLinkLease?.link === link else { return }
    link.isPaused = true

    guard shouldRender,
      hasPreparedState,
      presentationState.consumeOpportunity()
    else {
      return
    }

    if renderer.present(drawable: update.drawable) {
      presentationState.recordSubmissionSuccess()
      cancelPresentationRetryTimer()
    } else {
      guard let delay = presentationState.recordSubmissionFailure() else {
        lastRenderError = MetalTerminalRendererError.presentationSubmissionFailuresExhausted
        return
      }
      schedulePresentationRetry(after: delay)
    }
  }

  private func beginPresentationAttemptSeries() {
    presentationState.request()
  }

  private func cancelPresentationRetryTimer() {
    presentationRetryTimer?.invalidate()
    presentationRetryTimer = nil
    presentationRetryGeneration &+= 1
    presentationRetryScheduled = false
  }

  private func cancelPresentationRetries() {
    cancelPresentationRetryTimer()
    presentationState.cancel()
  }

  private func cancelPresentationOpportunity() {
    cancelPresentationRetryTimer()
    presentationState.cancelPending()
  }

  private func invalidatePreparedPresentation() {
    hasPreparedState = false
    cancelPresentationRetries()
    resetPreparationRecovery()
  }

  private func schedulePresentationRetry(after delay: TimeInterval) {
    guard shouldRender,
      hasPreparedState,
      presentationState.pending,
      !presentationRetryScheduled,
      !presentationState.exhausted
    else {
      return
    }

    presentationRetryScheduled = true
    let generation = presentationRetryGeneration
    presentationRetryTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) {
      [weak self] _ in
      Task { @MainActor [weak self] in
        self?.runPresentationRetry(generation: generation)
      }
    }
  }

  private func runPresentationRetry(generation: UInt64) {
    guard generation == presentationRetryGeneration else { return }
    presentationRetryTimer = nil
    presentationRetryScheduled = false
    guard shouldRender, hasPreparedState, presentationState.pending else { return }
    armMetalDisplayLink()
  }

  private func updateDrawableSize() {
    guard let metalLayer = layer as? CAMetalLayer else { return }
    let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 1
    metalLayer.contentsScale = scale
    metalLayer.drawableSize = convertToBacking(bounds).size
  }

  static func smokeTest() -> Bool {
    guard let device = MTLCreateSystemDefaultDevice() else { return false }
    do {
      let renderer = try MetalTerminalRenderer(device: device)
      return renderer.device.registryID == device.registryID
    } catch {
      return false
    }
  }
}

@MainActor
enum Pass6RegressionValidation {
  static func selfTest() -> Bool {
    supplementaryScalarResolutionSelfTest()
      && presentationRetryBudgetSelfTest()
      && presentationOpportunityStateSelfTest()
      && persistentPresentationFailureSelfTest()
      && persistentPreparationFailureIntegrationSelfTest()
      && transientDrawableRecoverySelfTest()
      && RendererValidation.inFlightVisibilityRecoverySelfTest()
      && RendererValidation.failedReplacementInvalidationSelfTest()
      && RustDisplayBridge.teardownReconnectStateSelfTest()
  }

  private static func supplementaryScalarResolutionSelfTest() -> Bool {
    // U+1F600 is a valid four-byte UTF-8 scalar and is available through the
    // macOS CoreText fallback stack. M001 does not claim emoji width/color
    // correctness yet, but it must not replace the scalar merely because its
    // UTF-16 representation uses a surrogate pair.
    TerminalFontResolver().canResolveScalarDirectly(0x1f600)
  }

  private static func presentationRetryBudgetSelfTest() -> Bool {
    var budget = PresentationRetryBudget()
    var claims = 0
    while budget.claimNextDelay() != nil {
      claims += 1
    }
    guard claims == 4, budget.exhausted, budget.claimNextDelay() == nil else {
      return false
    }
    budget.reset()
    return !budget.exhausted && budget.claimNextDelay() != nil
  }

  private static func presentationOpportunityStateSelfTest() -> Bool {
    // Repeated invalidations coalesce into one outstanding display-link
    // opportunity. A failed submission restores pending work without
    // leaving the link armed twice; a successful submission clears it.
    var state = PresentationOpportunityState()
    var armCount = 0
    for _ in 0..<1_000 {
      state.request()
      if state.armIfNeeded() {
        armCount += 1
      }
    }
    guard armCount == 1, state.pending, state.armed else { return false }
    guard state.consumeOpportunity() else { return false }
    state.markFailed()
    guard state.pending, !state.armed else { return false }
    guard state.armIfNeeded(), !state.armIfNeeded() else { return false }
    state.markPresented()
    return !state.pending && !state.armed
  }

  private static func persistentPresentationFailureSelfTest() -> Bool {
    var state = PresentationRecoveryState()
    var attempts = 0

    // A new frame or layout invalidation is represented by request(). It
    // must coalesce without replenishing the finite failure budget.
    for _ in 0...PresentationRetryBudget.maximumAutomaticRetries {
      for _ in 0..<1_000 {
        state.request()
      }
      guard state.armIfNeeded(), state.consumeOpportunity() else {
        return false
      }
      attempts += 1
      _ = state.recordSubmissionFailure()
    }

    guard attempts == 5,
      state.exhausted,
      state.retryBudget.exhausted,
      !state.armIfNeeded()
    else {
      return false
    }

    for _ in 0..<1_000 {
      state.request()
      guard !state.armIfNeeded() else { return false }
    }

    state.resetForLifecycleRecovery()
    state.request()
    guard !state.exhausted, state.armIfNeeded() else { return false }
    state.recordSubmissionSuccess()
    return !state.exhausted && !state.pending
  }

  private static func persistentPreparationFailureIntegrationSelfTest() -> Bool {
    // Exercise the same consumeBridgeFrame boundary: one initial attempt,
    // four delayed reprepare attempts, then a persistent latch. New
    // Candidate-D frames must not reach renderer.update after exhaustion.
    var state = PreparationRecoveryState()
    var preparationAttempts = 0
    func consumeCandidateDFrame() {
      guard state.canAttemptPreparation else { return }
      preparationAttempts += 1
    }

    for attempt in 0...PresentationRetryBudget.maximumAutomaticRetries {
      consumeCandidateDFrame()
      guard preparationAttempts == attempt + 1 else { return false }
      _ = state.recordFailure()
      if attempt < PresentationRetryBudget.maximumAutomaticRetries,
        state.exhausted
      {
        return false
      }
    }

    guard preparationAttempts == 5,
      state.exhausted,
      state.retryBudget.exhausted
    else {
      return false
    }

    for _ in 0..<1_000 {
      consumeCandidateDFrame()
    }
    guard preparationAttempts == 5 else { return false }
    state.resetForLifecycleRecovery()
    consumeCandidateDFrame()
    guard state.canAttemptPreparation, preparationAttempts == 6 else { return false }
    state.recordSuccess()
    return !state.exhausted
  }

  private static func transientDrawableRecoverySelfTest() -> Bool {
    guard let device = MTLCreateSystemDefaultDevice() else { return false }
    do {
      let renderer = try MetalTerminalRenderer(device: device)
      var damage = DamageMask()
      damage.mark(row: 0)
      var cell = SeyalPreparedCell()
      cell.scalar = UInt32(ascii: "A")
      cell.foreground = 0
      cell.background = 0
      cell.flags = 0
      cell.reserved = 0
      let cells = [cell]

      guard
        try cells.withUnsafeBufferPointer({ buffer in
          try renderer.update(
            frame: NativePreparedFrame(
              cells: buffer,
              generation: 1,
              rows: 1,
              columns: 1,
              damage: damage
            ),
            backingScale: 1,
            forceFullRebuild: true
          ) == .updated
        })
      else {
        return false
      }

      // Test-only acquisition exercises the same submission method used
      // by the production display-link callback.
      let cellSize = renderer.cellPixelSize(backingScale: 1)
      let layer = CAMetalLayer()
      layer.device = device
      layer.pixelFormat = .bgra8Unorm
      layer.framebufferOnly = true
      layer.maximumDrawableCount = 2
      layer.presentsWithTransaction = false
      layer.contentsScale = 1
      layer.bounds = CGRect(x: 0, y: 0, width: cellSize.width, height: cellSize.height)
      layer.drawableSize = CGSize(width: cellSize.width, height: cellSize.height)

      let completedBefore = renderer.stats.completedFrames
      guard
        RendererValidation.presentOnLayerForValidation(
          renderer: renderer,
          layer: layer
        )
      else { return false }
      let deadline = Date().addingTimeInterval(2)
      while renderer.stats.completedFrames == completedBefore && Date() < deadline {
        RunLoop.current.run(until: Date().addingTimeInterval(0.01))
      }
      return renderer.stats.completedFrames > completedBefore
    } catch {
      return false
    }
  }
}

extension UInt32 {
  fileprivate init(ascii character: Character) {
    self = character.unicodeScalars.first?.value ?? 0x20
  }
}
