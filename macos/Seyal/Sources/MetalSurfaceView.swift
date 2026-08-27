import AppKit
import Metal
@preconcurrency import QuartzCore

struct PresentationRetryBudget: Equatable {
    private static let delays: [TimeInterval] = [1.0 / 60.0, 0.05, 0.20, 0.75]
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
final class MetalSurfaceView: NSView, @MainActor CAMetalDisplayLinkDelegate {
    private let metalDevice: any MTLDevice
    private let renderer: MetalTerminalRenderer
    private var bridge: RustDisplayBridge?
    private var forceNextFrame = false
    private var hasPreparedState = false
    private var presentationState = PresentationOpportunityState()
    private var presentationRetryScheduled = false
    private var presentationRetryTimer: Timer?
    private var presentationRetryGeneration: UInt64 = 0
    private var presentationRetryBudget = PresentationRetryBudget()
    private var metalDisplayLinkLease: MetalDisplayLinkLease?
    private var preparationRetryTimer: Timer?
    private var preparationRetryGeneration: UInt64 = 0
    private var preparationRetryScheduled = false
    private var preparationRetryBudget = PresentationRetryBudget()
    private(set) var lastBridgeError: Int32?
    private(set) var lastRenderError: Error?

    override init(frame frameRect: NSRect) {
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
            }
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

    override func layout() {
        super.layout()
        updateDrawableSize()
        guard shouldRender,
              hasPreparedState,
              renderer.persistentDisplayFailure == nil
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

    private func updateVisibility() {
        let renderable = shouldRender
        if renderable {
            if let metalLayer = layer as? CAMetalLayer {
                installMetalDisplayLink(on: metalLayer)
            }
            forceNextFrame = true
            if bridge?.isConnected == false {
                _ = bridge?.start()
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
            if renderer.persistentDisplayFailure == nil {
                lastRenderError = nil
            }
            bridge?.publishCurrentFrame()
            if hasPreparedState {
                renderer.requestPresent()
                beginPresentationAttemptSeries()
                armMetalDisplayLink()
            }
        }
    }

    private func consumeBridgeFrame(_ bridgeFrame: SeyalPreparedFrame) {
        guard let frame = NativePreparedFrame(bridgeFrame: bridgeFrame) else {
            return
        }
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
                // Candidate-D can continue advancing while an exhausted GPU
                // display failure is latched. A successful CPU preparation must
                // not erase that asynchronous display diagnostic.
                if renderer.persistentDisplayFailure == nil {
                    lastRenderError = nil
                }
                resetPreparationRetries()
                if shouldRender, renderer.persistentDisplayFailure == nil {
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
            cancelPresentationRetries()
            renderer.invalidatePreparedState()
            forceNextFrame = true
            schedulePreparationRetry()
        }
    }

    private func schedulePreparationRetry() {
        guard shouldRender,
              !hasPreparedState,
              !preparationRetryScheduled,
              let delay = preparationRetryBudget.claimNextDelay()
        else {
            return
        }

        preparationRetryScheduled = true
        let generation = preparationRetryGeneration
        preparationRetryTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { [weak self] _ in
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
        preparationRetryBudget.reset()
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
            presentationState.markPresented()
            resetPresentationRetries()
        } else {
            presentationState.markFailed()
            schedulePresentationRetry()
        }
    }

    private func beginPresentationAttemptSeries() {
        presentationState.request()
        resetPresentationRetries()
    }

    private func resetPresentationRetries() {
        presentationRetryTimer?.invalidate()
        presentationRetryTimer = nil
        presentationRetryGeneration &+= 1
        presentationRetryScheduled = false
        presentationRetryBudget.reset()
    }

    private func cancelPresentationRetries() {
        presentationState.cancel()
        resetPresentationRetries()
    }

    private func invalidatePreparedPresentation() {
        hasPreparedState = false
        cancelPresentationRetries()
        resetPreparationRetries()
    }

    private func schedulePresentationRetry() {
        guard shouldRender,
              hasPreparedState,
              presentationState.pending,
              !presentationRetryScheduled,
              let delay = presentationRetryBudget.claimNextDelay()
        else {
            return
        }

        presentationRetryScheduled = true
        let generation = presentationRetryGeneration
        presentationRetryTimer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { [weak self] _ in
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
            && preparationRetryBudgetSelfTest()
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

    private static func preparationRetryBudgetSelfTest() -> Bool {
        // Model repeated preparation failures without resetting the series.
        // Four delayed attempts are allowed; a persistent failure must not
        // schedule a fifth attempt or restart at the first delay.
        var budget = PresentationRetryBudget()
        var delayedAttempts = 0
        for _ in 0..<8 {
            guard budget.claimNextDelay() != nil else { break }
            delayedAttempts += 1
        }
        return delayedAttempts == 4
            && budget.exhausted
            && budget.claimNextDelay() == nil
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

            guard try cells.withUnsafeBufferPointer({ buffer in
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
            }) else {
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
            guard RendererValidation.presentOnLayerForValidation(
                renderer: renderer,
                layer: layer
            ) else { return false }
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

private extension UInt32 {
    init(ascii character: Character) {
        self = character.unicodeScalars.first?.value ?? 0x20
    }
}
