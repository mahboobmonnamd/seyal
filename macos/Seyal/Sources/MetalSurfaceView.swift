import AppKit
import Metal
import QuartzCore

@MainActor
final class MetalSurfaceView: NSView {
    private let metalDevice: any MTLDevice

    override init(frame frameRect: NSRect) {
        guard let device = MTLCreateSystemDefaultDevice() else {
            fatalError("Seyal requires a Metal-capable macOS device")
        }

        metalDevice = device
        super.init(frame: frameRect)
        wantsLayer = true

        guard let metalLayer = layer as? CAMetalLayer else {
            fatalError("MetalSurfaceView backing layer must be CAMetalLayer")
        }
        metalLayer.device = device
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = true
        updateDrawableSize()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("Seyal Issue #10 uses a programmatic AppKit surface")
    }

    override func makeBackingLayer() -> CALayer {
        CAMetalLayer()
    }

    override func layout() {
        super.layout()
        updateDrawableSize()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateDrawableSize()
    }

    private func updateDrawableSize() {
        guard let metalLayer = layer as? CAMetalLayer else { return }
        metalLayer.drawableSize = convertToBacking(bounds).size
    }

    static func smokeTest() -> Bool {
        guard let device = MTLCreateSystemDefaultDevice() else { return false }
        let layer = CAMetalLayer()
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        return layer.device != nil
    }
}
