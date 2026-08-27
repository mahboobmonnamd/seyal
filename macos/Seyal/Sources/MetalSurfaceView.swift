import AppKit
import Metal
import QuartzCore

@MainActor
final class MetalSurfaceView: NSView {
    private let metalDevice: any MTLDevice
    private let renderer: MetalTerminalRenderer
    private var bridge: RustDisplayBridge?
    private var forceNextFrame = false
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
        renderer.requestPresent()
        presentPreparedState()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateDrawableSize()
        forceNextFrame = true
        bridge?.publishCurrentFrame()
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

    private func updateVisibility() {
        let visible = window.map {
            !$0.isMiniaturized && $0.occlusionState.contains(.visible)
        } ?? false
        let shouldRender = visible && !isHidden
        if shouldRender {
            forceNextFrame = true
            if bridge?.isConnected == false {
                _ = bridge?.start()
            }
        }
        renderer.setVisible(shouldRender)
        if shouldRender {
            bridge?.publishCurrentFrame()
            renderer.requestPresent()
            presentPreparedState()
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
                lastRenderError = nil
                presentPreparedState()
            }
        } catch {
            lastRenderError = error
        }
    }

    private func presentPreparedState() {
        guard let metalLayer = layer as? CAMetalLayer else { return }
        _ = renderer.present(layer: metalLayer)
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
