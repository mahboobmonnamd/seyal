import AppKit
import Metal
import QuartzCore

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

@MainActor
final class MetalSurfaceView: NSView {
    private let metalDevice: any MTLDevice
    private let renderer: MetalTerminalRenderer
    private var bridge: RustDisplayBridge?
    private var forceNextFrame = false
    private var hasPreparedState = false
    private var presentationPending = false
    private var presentationRetryScheduled = false
    private var presentationRetryTimer: Timer?
    private var presentationRetryGeneration: UInt64 = 0
    private var presentationRetryBudget = PresentationRetryBudget()
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
        guard shouldRender, hasPreparedState else { return }
        renderer.requestPresent()
        beginPresentationAttemptSeries()
        presentPreparedState()
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
            forceNextFrame = true
            if bridge?.isConnected == false {
                _ = bridge?.start()
            }
        } else {
            invalidatePreparedPresentation()
        }

        renderer.setVisible(renderable)
        if renderable {
            // Showing reconstructs from the latest committed Candidate-D state.
            // It never depends on stale drawable contents or PTY-byte replay.
            bridge?.publishCurrentFrame()
            if hasPreparedState {
                renderer.requestPresent()
                beginPresentationAttemptSeries()
                presentPreparedState()
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
                lastRenderError = nil
                resetPreparationRetries()
                if shouldRender {
                    beginPresentationAttemptSeries()
                    presentPreparedState()
                }
            }
        } catch {
            lastRenderError = error
            // Renderer preparation is incremental for damage efficiency.  A
            // failed replacement must not leave a partially updated live
            // buffer eligible for a later present.
            hasPreparedState = false
            invalidatePreparedPresentation()
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

    /// Try to present the already prepared current frame. A temporary drawable
    /// miss or command-buffer construction failure does not require new terminal
    /// output: the same prepared state receives a small bounded retry series.
    /// The series is coalesced and finite, so persistent GPU/surface failure
    /// cannot create a display-link-style busy loop.
    private func presentPreparedState() {
        guard shouldRender,
              hasPreparedState,
              presentationPending,
              let metalLayer = layer as? CAMetalLayer
        else {
            return
        }

        if renderer.present(layer: metalLayer) {
            presentationPending = false
            resetPresentationRetries()
        } else {
            schedulePresentationRetry()
        }
    }

    private func beginPresentationAttemptSeries() {
        presentationPending = true
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
        presentationPending = false
        resetPresentationRetries()
        resetPreparationRetries()
    }

    private func invalidatePreparedPresentation() {
        hasPreparedState = false
        cancelPresentationRetries()
    }

    private func schedulePresentationRetry() {
        guard shouldRender,
              hasPreparedState,
              presentationPending,
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
        guard shouldRender, hasPreparedState, presentationPending else { return }
        presentPreparedState()
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
            && transientDrawableRecoverySelfTest()
            && RendererValidation.inFlightVisibilityRecoverySelfTest()
            && RendererValidation.failedReplacementInvalidationSelfTest()
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

            // Simulate the production nextDrawable()==nil branch. No second
            // update/current-frame publication follows: the already prepared
            // static frame must remain presentable on a later opportunity.
            let missesBefore = renderer.stats.drawableMisses
            renderer.handleDrawableUnavailable()
            guard renderer.stats.drawableMisses == missesBefore + 1 else { return false }

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
            guard renderer.present(layer: layer) else { return false }
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
