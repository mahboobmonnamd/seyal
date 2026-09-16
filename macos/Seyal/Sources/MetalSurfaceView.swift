import AppKit
import Metal
@preconcurrency import QuartzCore

@MainActor
class MetalSurfaceView: NSView, CAMetalDisplayLinkDelegate {
  /// How AppKit installs the surface presenter.
  enum Installation: Equatable {
    /// Production Metal display path (`CAMetalLayer` + Runtime bridge).
    case fullDisplay
    /// SPEC-009 §10 first-responder / AX / IME only: plain layer, no Runtime
    /// bridge, no CAMetalLayer drawables. Used by Pass 9 native_ready probe so
    /// the soak's MetalTerminalRenderer remains the sole display presenter.
    case nativeInteractionProbe
  }

  let paneID: String
  let requestedExecutionIdentity: String?
  let allowsImplicitExecutionBootstrap: Bool
  let installation: Installation
  let metalDevice: any MTLDevice
  let renderer: MetalTerminalRenderer
  var bridge: RustDisplayBridge?
  lazy var bridgeRecoveryCoordinator = RuntimeLifecycleRecoveryCoordinator(
    clock: { CACurrentMediaTime() },
    scheduler: { delay, operation in
      let timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
        seyalRunAsMainActorFromMainQueue { operation() }
      }
      let timerBox = RuntimeRecoveryTimerBox(timer: timer)
      return { timerBox.timer.invalidate() }
    },
    launcher: { [weak self] in self?.bridge?.launchBundledRuntime() },
    attempt: { [requestedExecutionIdentity, allowsImplicitExecutionBootstrap] in
      openRuntimeRecoveryHandle(
        executionIdentity: requestedExecutionIdentity,
        allowsImplicitExecutionBootstrap: allowsImplicitExecutionBootstrap
      )
    },
    handleAdopter: { [weak self] opened in
      self?.bridge?.adoptRecoveredHandle(opened) ?? false
    }
  )
  var runtimeRecoveryState: RuntimeRecoveryState { bridgeRecoveryCoordinator.state }
  /// When true, the surface still supports first-responder / AX / IME restore
  /// (SPEC §10) but does not begin automatic Runtime recovery. Used by the
  /// Pass 9 native_ready probe so it does not open a second client alongside
  /// the qualification soak bridge.
  var suppressesAutomaticBridgeRecovery = false
  var forceNextFrame = false
  var hasPreparedState = false
  var presentationState = PresentationRecoveryState()
  var presentationRetryScheduled = false
  var presentationRetryTimer: Timer?
  var presentationRetryGeneration: UInt64 = 0
  var renderable = false
  var metalDisplayLinkLease: MetalDisplayLinkLease?
  /// Identity for CAMetalDisplayLink hops without capturing `@MainActor self`
  /// in a way that inserts `assumeIsolated` under Xcode 16.4.
  nonisolated(unsafe) var displayLinkHopTarget: Unmanaged<MetalSurfaceView>?
  var preparationRetryTimer: Timer?
  var preparationRetryGeneration: UInt64 = 0
  var preparationRetryScheduled = false
  var preparationState = PreparationRecoveryState()
  var lastAlternateScreen: Bool?
  var isDetachingRuntimeConnection = false
  var lastBridgeError: Int32?
  var lastRenderError: Error?
  var historyRanges: [PaneBlockKey: NativeHistoryRange] = [:]
  var lastProposedGeometry = CGRect.null
  var proposingGeometry = false

  override convenience init(frame frameRect: NSRect) {
    self.init(frame: frameRect, paneID: "unbound")
  }

  init(
    frame frameRect: NSRect,
    paneID: String,
    executionIdentity: String? = nil,
    allowsImplicitExecutionBootstrap: Bool = true,
    terminalFont: SeyalResolvedFontSpec = .canonicalTerminal,
    installation: Installation = .fullDisplay
  ) {
    self.paneID = paneID
    self.requestedExecutionIdentity = executionIdentity
    self.allowsImplicitExecutionBootstrap = allowsImplicitExecutionBootstrap
    self.installation = installation
    guard let device = MTLCreateSystemDefaultDevice() else {
      fatalError("Seyal requires a Metal-capable macOS device")
    }
    let renderer: MetalTerminalRenderer
    do {
      renderer = try MetalTerminalRenderer(device: device, terminalFont: terminalFont)
    } catch {
      fatalError("Seyal permanent Metal renderer initialization failed: \(error)")
    }

    metalDevice = device
    self.renderer = renderer
    super.init(frame: frameRect)
    displayLinkHopTarget = Unmanaged.passUnretained(self)
    wantsLayer = true

    switch installation {
    case .fullDisplay:
      guard let metalLayer = layer as? CAMetalLayer else {
        fatalError("MetalSurfaceView backing layer must be CAMetalLayer")
      }
      metalLayer.device = device
      metalLayer.pixelFormat = .bgra8Unorm
      metalLayer.framebufferOnly = true
      metalLayer.maximumDrawableCount = 2
      metalLayer.presentsWithTransaction = false
      metalLayer.isOpaque = true
      updateDrawableSize()

      // No dedicated GPU surface resources are retained before the view is
      // actually visible. Candidate-D state may still advance independently.
      renderer.setVisible(false)
      let hopTarget = displayLinkHopTarget!
      renderer.onNeedsCurrentFrame = {
        seyalRunAsMainActorFromMainQueue {
          hopTarget.takeUnretainedValue().bridge?.publishCurrentFrame()
        }
      }
      renderer.onPersistentDisplayFailure = { error in
        seyalRunAsMainActorFromMainQueue {
          hopTarget.takeUnretainedValue().lastRenderError = error
        }
      }

      let bridge = RustDisplayBridge(
        onFrame: { [weak self] frame in
          self?.consumeBridgeFrame(frame)
        },
        onError: { [weak self] code in
          self?.lastBridgeError = code
          self?.terminalBridgeDidFail(code)
        },
        onStatusChanged: { [weak self] in
          self?.terminalBridgeStatusDidChange()
        },
        onTimeline: { [weak self] in
          self?.onTimelineChanged?()
        },
        onHistory: { [weak self] range in
          guard let self else { return }
          // Retain rows before chrome publishes clips. `setTranscriptFrame`
          // only re-encodes ranges already stored here; dropping this store
          // leaves Block bodies empty even after Runtime history arrives.
          self.retainHistoryRange(range)
          if let onHistoryRangeChanged {
            onHistoryRangeChanged(range)
          } else {
            self.renderHistoryRange(range)
          }
        },
        onComposerResult: { [weak self] result in
          self?.onComposerResultChanged?(result)
        },
        paneID: paneID,
        executionIdentity: executionIdentity,
        allowsImplicitExecutionBootstrap: allowsImplicitExecutionBootstrap
      )
      bridge.onCopiedText = { text in
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
      }
      self.bridge = bridge
      // A production surface must not perform a synchronous pre-attempt on the
      // AppKit thread. Visibility starts the one authoritative recovery episode
      // so startup, retries and cancellation share the exact seven-attempt/
      // one-second contract instead of creating an eighth attempt with a fresh
      // timeout before the lifecycle coordinator begins.

    case .nativeInteractionProbe:
      // SPEC §10 probe: InteractiveMetalSurfaceView restore only. The Pass 9
      // soak owns the live Runtime/Metal presenter; this view must not allocate
      // CAMetalLayer drawables or a second client bridge.
      suppressesAutomaticBridgeRecovery = true
      renderer.setVisible(false)
    }
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) {
    fatalError("Seyal uses a programmatic AppKit/Metal surface")
  }

  override func makeBackingLayer() -> CALayer {
    switch installation {
    case .fullDisplay:
      CAMetalLayer()
    case .nativeInteractionProbe:
      CALayer()
    }
  }

  /// Narrow subclass hooks for Pass 7 presentation-only failure/focus state.
  /// They never transfer PTY, VT, grid or renderer authority into AppKit.
  func terminalBridgeDidFail(_ code: Int32) {
    _ = code
  }

  func terminalBridgeStatusDidChange() {
    refreshRecoveryAccessibilityValue()
    guard !isDetachingRuntimeConnection else { return }
    if bridge?.isConnected == true {
      // Propose from `layout()` only. `proposeGeometry` always finishes with
      // `onStatusChanged`, so calling it here re-enters this method until the
      // stack overflows (EXC_BAD_ACCESS on the guard page).
      needsLayout = true
      return
    }
    lastProposedGeometry = .null
    // History/composer/display correlations are disposable connection state;
    // logical pane and Block identity remain owned by Runtime and are not
    // cleared here.
    historyRanges.removeAll(keepingCapacity: false)
    invalidatePreparedPresentation()
    startAutomaticBridgeRecoveryIfNeeded()
  }

  /// SPEC-009 §10: renderer-ready → native interaction before `Usable`.
  /// Base Metal surface has no text-input/first-responder seam; subclasses that
  /// own `NSTextInputClient` must restore focus/AX/IME here.
  @discardableResult
  func restoreNativeInteractionAfterRendererReady() -> Bool {
    true
  }

  /// Presentation-only notification. Runtime/Metal remains authoritative;
  /// AppKit uses this to switch the surrounding Pane chrome.
  var onAlternateScreenChanged: ((Bool) -> Void)?
  var onFrameChanged: ((NativePreparedFrame) -> Void)?
  var onTimelineChanged: (() -> Void)?
  var onHistoryRangeChanged: ((NativeHistoryRange) -> Void)?
  var onComposerResultChanged: ((NativeComposerResult) -> Void)?

  var terminalBridgeIsConnected: Bool {
    bridge?.isConnected == true
  }

  /// A command entered while disconnected is an explicit recovery action, but
  /// it must never synchronously connect/handshake/attach on the AppKit thread.
  /// The Pane composer keeps the draft when this returns false; the coordinator
  /// owns the bounded episode and the user can submit once the surface is usable.
  @discardableResult
  override func layout() {
    super.layout()
    if suppressesAutomaticBridgeRecovery {
      // SPEC §10 probe: keep a 1×1 drawable so key-window/first-responder
      // restore does not allocate full-size CAMetalLayer backings each cycle.
      if let metalLayer = layer as? CAMetalLayer {
        metalLayer.drawableSize = CGSize(width: 1, height: 1)
      }
      return
    }
    updateDrawableSize()
    proposeCurrentGeometry()
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
      NotificationCenter.default.removeObserver(
        self,
        name: NSWindow.didBecomeKeyNotification,
        object: window
      )
      NotificationCenter.default.removeObserver(
        self,
        name: NSWindow.didBecomeMainNotification,
        object: window
      )
    }
    if newWindow == nil {
      // Suppress status-driven reconnect before stop() publishes its
      // disconnected transition. Teardown is detach-only and must not create
      // a replacement foreground recovery episode.
      detachRuntimeConnectionForApplicationTermination()
    }
    super.viewWillMove(toWindow: newWindow)
  }

  /// Detaches the disposable GUI-side Runtime client before the application
  /// exits. The Runtime/helper intentionally survives GUI lifetime, but its
  /// controller lease must be released before a later Seyal launch attempts
  /// to reacquire the same execution.
  func detachRuntimeConnectionForApplicationTermination() {
    isDetachingRuntimeConnection = true
    renderable = false
    renderer.setVisible(false)
    invalidatePreparedPresentation()
    invalidateMetalDisplayLink()
    cancelBridgeReconnect()
    bridge?.stop()
  }

  override func viewDidMoveToWindow() {
    super.viewDidMoveToWindow()
    if window != nil {
      isDetachingRuntimeConnection = false
    }
    if let window {
      NotificationCenter.default.addObserver(
        self,
        selector: #selector(windowOcclusionChanged),
        name: NSWindow.didChangeOcclusionStateNotification,
        object: window
      )
      NotificationCenter.default.addObserver(
        self,
        selector: #selector(windowActivationChanged),
        name: NSWindow.didBecomeKeyNotification,
        object: window
      )
      NotificationCenter.default.addObserver(
        self,
        selector: #selector(windowActivationChanged),
        name: NSWindow.didBecomeMainNotification,
        object: window
      )
    }
    updateDrawableSize()
    updateVisibility()
    // The production shell installs its content view before ordering the
    // window front. Recheck after AppKit completes that ordering so the
    // visibility-gated Runtime recovery episode cannot be stranded at
    // `disconnected` during launch.
    DispatchQueue.main.async { [weak self] in
      self?.updateVisibility()
    }
  }

  @objc func windowOcclusionChanged() {
    updateVisibility()
  }

  @objc func windowActivationChanged() {
    updateVisibility()
  }

  var shouldRender: Bool {
    guard shouldAttachRuntime else { return false }
    guard let window else { return false }
    return window.occlusionState.contains(.visible)
  }

  /// Runtime attachment is a lifecycle concern, not a Metal presentation
  /// concern. AppKit may report a stale/non-visible occlusion state while a
  /// newly reopened window is already visible and focusable. Gating attach on
  /// that state strands the pane with no Runtime, no Metal frame, and no
  /// working Enter key until an unrelated window/occlusion notification.
  /// A surface already installed in a non-miniaturized window is an eligible
  /// foreground pane even while AppKit is still settling visibility or an
  /// ancestor's occlusion bookkeeping.
  var shouldAttachRuntime: Bool {
    guard let window else { return false }
    return !window.isMiniaturized
  }

  func cancelBridgeReconnect() {
    bridgeRecoveryCoordinator.cancel()
  }

  func startAutomaticBridgeRecoveryIfNeeded() {
    guard !suppressesAutomaticBridgeRecovery,
      !isDetachingRuntimeConnection,
      shouldAttachRuntime,
      bridge?.isConnected == false,
      // stop() keeps the old clientHandle until both dispatch-source cancel
      // handlers complete. Waiting for zero prevents consuming a retry on our
      // own in-progress detach/controller cleanup.
      bridge?.clientHandle == 0,
      !bridgeRecoveryCoordinator.isActive,
      runtimeRecoveryState.stage != .exhausted,
      runtimeRecoveryState.stage != .blocked
    else { return }
    bridgeRecoveryCoordinator.beginEpisode()
  }

  /// Explicit user retry starts a new bounded foreground recovery episode.
  /// Automatic exhaustion never invokes this method recursively.
  @discardableResult
  func retryRuntimeConnection() -> Bool {
    guard shouldAttachRuntime,
      bridge?.isConnected != true,
      bridge?.clientHandle == 0,
      runtimeRecoveryState.stage != .blocked
    else { return bridge?.isConnected == true }
    bridgeRecoveryCoordinator.retry()
    return bridge?.isConnected == true
  }

  /// AppKit can finish attaching a scroll-view sibling to its window after
  /// `viewDidMoveToWindow` has already run. Re-evaluate the lifecycle boundary
  /// after the shell's window has been ordered front so Runtime discovery is
  /// never left stranded in `.disconnected`.
  func activateRuntimeAfterWindowPresentation() {
    updateVisibility()
    startAutomaticBridgeRecoveryIfNeeded()
    refreshRecoveryAccessibilityValue()
  }

  func updateVisibility() {
    let renderable = shouldRender
    let becameRenderable = renderable && !self.renderable
    self.renderable = renderable
    if suppressesAutomaticBridgeRecovery {
      // SPEC §10 probe surfaces: first-responder / AX / IME only — no Metal
      // display-link or automatic Runtime recovery alongside the soak bridge.
      invalidateMetalDisplayLink()
      renderer.setVisible(false)
      return
    }
    if renderable {
      if let metalLayer = layer as? CAMetalLayer {
        installMetalDisplayLink(on: metalLayer)
      }
      forceNextFrame = true
      startAutomaticBridgeRecoveryIfNeeded()
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
    } else if shouldAttachRuntime {
      // The window can be attachable before AppKit publishes a reliable
      // occlusion state. Keep Runtime recovery independent from presentation;
      // the renderer remains hidden until a real visible-frame opportunity.
      startAutomaticBridgeRecoveryIfNeeded()
    }
  }
}
