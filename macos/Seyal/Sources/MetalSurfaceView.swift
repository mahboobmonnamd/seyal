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
        guard shouldRender else { return }
        renderer.requestPresent()
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
        }
        renderer.setVisible(renderable)
        if renderable {
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
                if shouldRender {
                    presentPreparedState()
                }
            }
        } catch {
            lastRenderError = error
        }
    }

    private func presentPreparedState() {
        guard shouldRender, let metalLayer = layer as? CAMetalLayer else { return }
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
