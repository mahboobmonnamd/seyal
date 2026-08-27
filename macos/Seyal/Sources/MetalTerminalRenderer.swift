import Foundation
import Metal
import QuartzCore

private let preparedBoldFlag: UInt16 = 1 << 0
private let preparedUnderlineFlag: UInt16 = 1 << 1
private let instanceGlyphFlag: UInt32 = 1 << 0
private let instanceUnderlineFlag: UInt32 = 1 << 1
private let instanceCursorFlag: UInt32 = 1 << 2

private struct TerminalInstance {
    var origin: SIMD2<Float>
    var size: SIMD2<Float>
    var uvRect: SIMD4<Float>
    var foreground: UInt32
    var background: UInt32
    var flags: UInt32
    var atlasSlice: UInt32
}

struct DamageMask: Equatable {
    var word0: UInt64 = 0
    var word1: UInt64 = 0
    var word2: UInt64 = 0
    var word3: UInt64 = 0

    var isEmpty: Bool {
        word0 == 0 && word1 == 0 && word2 == 0 && word3 == 0
    }

    mutating func formUnion(_ other: DamageMask) {
        word0 |= other.word0
        word1 |= other.word1
        word2 |= other.word2
        word3 |= other.word3
    }

    mutating func markAll(rows: Int) {
        word0 = 0
        word1 = 0
        word2 = 0
        word3 = 0
        for row in 0..<min(rows, 256) {
            mark(row: row)
        }
    }

    mutating func mark(row: Int) {
        guard row >= 0, row < 256 else { return }
        let bit = UInt64(1) << UInt64(row & 63)
        switch row >> 6 {
        case 0: word0 |= bit
        case 1: word1 |= bit
        case 2: word2 |= bit
        default: word3 |= bit
        }
    }

    func contains(row: Int) -> Bool {
        guard row >= 0, row < 256 else { return false }
        let bit = UInt64(1) << UInt64(row & 63)
        switch row >> 6 {
        case 0: return word0 & bit != 0
        case 1: return word1 & bit != 0
        case 2: return word2 & bit != 0
        default: return word3 & bit != 0
        }
    }
}

struct NativePreparedFrame {
    let cells: UnsafeBufferPointer<SeyalPreparedCell>
    let generation: UInt64
    let rows: Int
    let columns: Int
    let cursorRow: Int
    let cursorColumn: Int
    let cursorVisible: Bool
    let alternateScreen: Bool
    let fullRebuild: Bool
    let damage: DamageMask

    init?(bridgeFrame: SeyalPreparedFrame) {
        let rows = Int(bridgeFrame.rows)
        let columns = Int(bridgeFrame.columns)
        let count = Int(bridgeFrame.cell_count)
        guard rows > 0,
              columns > 0,
              rows <= 256,
              columns <= 512,
              count == rows * columns,
              let pointer = bridgeFrame.cells
        else {
            return nil
        }
        cells = UnsafeBufferPointer(start: pointer, count: count)
        generation = bridgeFrame.generation
        self.rows = rows
        self.columns = columns
        cursorRow = Int(bridgeFrame.cursor_row)
        cursorColumn = Int(bridgeFrame.cursor_column)
        cursorVisible = bridgeFrame.cursor_visible != 0
        alternateScreen = bridgeFrame.alternate_screen != 0
        fullRebuild = bridgeFrame.full_rebuild != 0
        damage = DamageMask(
            word0: bridgeFrame.damage_word0,
            word1: bridgeFrame.damage_word1,
            word2: bridgeFrame.damage_word2,
            word3: bridgeFrame.damage_word3
        )
    }

    init(
        cells: UnsafeBufferPointer<SeyalPreparedCell>,
        generation: UInt64,
        rows: Int,
        columns: Int,
        cursorRow: Int = 0,
        cursorColumn: Int = 0,
        cursorVisible: Bool = false,
        alternateScreen: Bool = false,
        fullRebuild: Bool = true,
        damage: DamageMask = DamageMask()
    ) {
        self.cells = cells
        self.generation = generation
        self.rows = rows
        self.columns = columns
        self.cursorRow = cursorRow
        self.cursorColumn = cursorColumn
        self.cursorVisible = cursorVisible
        self.alternateScreen = alternateScreen
        self.fullRebuild = fullRebuild
        self.damage = damage
    }
}

enum MetalTerminalRendererError: Error {
    case unavailableCommandQueue
    case unavailableLibrary
    case unavailableShader
    case unavailablePipeline
    case unavailableSampler
    case unavailableBuffer
    case invalidFrame
    case invalidInstanceLayout
    case glyphAtlas(GlyphAtlasError)
}

enum RendererUpdateResult: Equatable {
    case updated
    case deferred
}

struct MetalRendererStats: Equatable {
    var submittedFrames: UInt64 = 0
    var completedFrames: UInt64 = 0
    var coalescedFrames: UInt64 = 0
    var drawableMisses: UInt64 = 0
    var fullRebuilds: UInt64 = 0
    var rebuiltRows: UInt64 = 0
    var rebuiltCells: UInt64 = 0
    var instanceBufferAllocations: UInt64 = 0
    var instanceBytes: UInt64 = 0
}

@MainActor
final class MetalTerminalRenderer {
    static let maximumFramesInFlight = 1

    let device: MTLDevice
    private let commandQueue: MTLCommandQueue
    private let pipeline: MTLRenderPipelineState
    private let sampler: MTLSamplerState
    private let glyphAtlas: GlyphAtlas

    private var instanceBuffer: MTLBuffer?
    private var instanceCount = 0
    private var currentRows = 0
    private var currentColumns = 0
    private var currentMetrics: TerminalFontMetrics?
    private var currentScale: CGFloat = 0
    private var currentAlternateScreen = false
    private var framesInFlight = 0
    private var deferredDamage = DamageMask()
    private var deferredNeedsFullRebuild = false
    private var releaseWhenIdle = false
    private var visible = true
    private var needsPresent = false
    private var needsCurrentFrameWhenIdle = false

    private(set) var stats = MetalRendererStats()
    var onNeedsCurrentFrame: (() -> Void)?

    init(device: MTLDevice) throws {
        self.device = device
        guard MemoryLayout<TerminalInstance>.stride == 48 else {
            throw MetalTerminalRendererError.invalidInstanceLayout
        }
        guard let commandQueue = device.makeCommandQueue() else {
            throw MetalTerminalRendererError.unavailableCommandQueue
        }
        self.commandQueue = commandQueue
        commandQueue.label = "Seyal Terminal Command Queue"

        let library: MTLLibrary
        do {
            library = try device.makeDefaultLibrary(bundle: .main)
        } catch {
            throw MetalTerminalRendererError.unavailableLibrary
        }
        guard let vertex = library.makeFunction(name: "seyal_terminal_vertex"),
              let fragment = library.makeFunction(name: "seyal_terminal_fragment")
        else {
            throw MetalTerminalRendererError.unavailableShader
        }
        let pipelineDescriptor = MTLRenderPipelineDescriptor()
        pipelineDescriptor.label = "Seyal Terminal Pipeline"
        pipelineDescriptor.vertexFunction = vertex
        pipelineDescriptor.fragmentFunction = fragment
        pipelineDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        do {
            pipeline = try device.makeRenderPipelineState(descriptor: pipelineDescriptor)
        } catch {
            throw MetalTerminalRendererError.unavailablePipeline
        }

        let samplerDescriptor = MTLSamplerDescriptor()
        samplerDescriptor.minFilter = .linear
        samplerDescriptor.magFilter = .linear
        samplerDescriptor.sAddressMode = .clampToEdge
        samplerDescriptor.tAddressMode = .clampToEdge
        guard let sampler = device.makeSamplerState(descriptor: samplerDescriptor) else {
            throw MetalTerminalRendererError.unavailableSampler
        }
        self.sampler = sampler
        glyphAtlas = GlyphAtlas(device: device)
    }

    var hasDedicatedSurfaceResources: Bool {
        instanceBuffer != nil || glyphAtlas.estimatedResidentBytes != 0
    }

    var glyphStats: GlyphAtlasStats {
        glyphAtlas.stats
    }

    var estimatedDedicatedGPUBytes: Int {
        Int(stats.instanceBytes) + glyphAtlas.estimatedResidentBytes
    }

    func cellPixelSize(backingScale: CGFloat) -> (width: Int, height: Int) {
        let metrics = glyphAtlas.metrics(backingScale: max(backingScale, 1))
        return (metrics.cellWidth, metrics.cellHeight)
    }

    func requestPresent() {
        guard visible, instanceBuffer != nil else { return }
        needsPresent = true
    }

    func setVisible(_ value: Bool) {
        guard visible != value else { return }
        visible = value
        if value {
            deferredNeedsFullRebuild = true
            needsCurrentFrameWhenIdle = true
            if framesInFlight == 0 {
                requestCurrentFrameIfNeeded()
            }
        } else {
            needsPresent = false
            if framesInFlight == 0 {
                releaseDedicatedResources()
            } else {
                releaseWhenIdle = true
            }
        }
    }

    func update(
        frame: NativePreparedFrame,
        backingScale: CGFloat,
        forceFullRebuild: Bool = false
    ) throws -> RendererUpdateResult {
        guard frame.rows > 0,
              frame.columns > 0,
              frame.rows <= 256,
              frame.columns <= 512,
              frame.cells.count == frame.rows * frame.columns,
              frame.cursorRow >= 0,
              frame.cursorRow < frame.rows,
              frame.cursorColumn >= 0,
              frame.cursorColumn < frame.columns
        else {
            throw MetalTerminalRendererError.invalidFrame
        }

        var incomingDamage = frame.damage
        if frame.fullRebuild || forceFullRebuild {
            incomingDamage.markAll(rows: frame.rows)
        }

        if !visible {
            deferredNeedsFullRebuild = true
            deferredDamage.formUnion(incomingDamage)
            stats.coalescedFrames &+= 1
            return .deferred
        }

        if framesInFlight >= Self.maximumFramesInFlight {
            deferredDamage.formUnion(incomingDamage)
            deferredNeedsFullRebuild = deferredNeedsFullRebuild
                || frame.fullRebuild
                || forceFullRebuild
            needsCurrentFrameWhenIdle = true
            stats.coalescedFrames &+= 1
            return .deferred
        }

        let scale = max(backingScale, 1)
        let metrics = glyphAtlas.metrics(backingScale: scale)
        let geometryChanged = currentRows != frame.rows || currentColumns != frame.columns
        let scaleChanged = currentScale != scale || currentMetrics != metrics
        let screenChanged = currentAlternateScreen != frame.alternateScreen
        var fullRebuild = forceFullRebuild
            || frame.fullRebuild
            || geometryChanged
            || scaleChanged
            || screenChanged
            || deferredNeedsFullRebuild
            || instanceBuffer == nil

        var damage = incomingDamage
        damage.formUnion(deferredDamage)
        if fullRebuild {
            damage.markAll(rows: frame.rows)
        }

        if scaleChanged {
            glyphAtlas.resetWhenGPUIdle()
        }
        if geometryChanged || instanceBuffer == nil {
            try allocateInstanceBuffer(rows: frame.rows, columns: frame.columns)
            fullRebuild = true
            damage.markAll(rows: frame.rows)
        }

        do {
            try rebuildRows(
                frame: frame,
                damage: damage,
                metrics: metrics,
                backingScale: scale
            )
        } catch GlyphAtlasError.capacityExceeded {
            glyphAtlas.resetWhenGPUIdle()
            var allRows = DamageMask()
            allRows.markAll(rows: frame.rows)
            do {
                try rebuildRows(
                    frame: frame,
                    damage: allRows,
                    metrics: metrics,
                    backingScale: scale
                )
            } catch let error as GlyphAtlasError {
                throw MetalTerminalRendererError.glyphAtlas(error)
            }
            fullRebuild = true
            damage = allRows
        } catch let error as GlyphAtlasError {
            throw MetalTerminalRendererError.glyphAtlas(error)
        }

        do {
            _ = try glyphAtlas.ensureTextureForRendering()
        } catch let error as GlyphAtlasError {
            throw MetalTerminalRendererError.glyphAtlas(error)
        }

        currentRows = frame.rows
        currentColumns = frame.columns
        currentMetrics = metrics
        currentScale = scale
        currentAlternateScreen = frame.alternateScreen
        deferredDamage = DamageMask()
        deferredNeedsFullRebuild = false
        needsCurrentFrameWhenIdle = false
        needsPresent = true

        if fullRebuild {
            stats.fullRebuilds &+= 1
        }
        for row in 0..<frame.rows where damage.contains(row: row) {
            stats.rebuiltRows &+= 1
            stats.rebuiltCells &+= UInt64(frame.columns)
        }
        return .updated
    }

    @discardableResult
    func present(layer: CAMetalLayer) -> Bool {
        guard visible,
              needsPresent,
              framesInFlight == 0,
              let instanceBuffer,
              instanceCount > 0,
              let atlasTexture = glyphAtlas.texture
        else {
            return false
        }
        guard let drawable = layer.nextDrawable() else {
            handleDrawableUnavailable()
            return false
        }
        guard let commandBuffer = makeCommandBuffer(
            target: drawable.texture,
            instanceBuffer: instanceBuffer,
            atlasTexture: atlasTexture
        ) else {
            deferredNeedsFullRebuild = true
            needsCurrentFrameWhenIdle = true
            return false
        }

        framesInFlight = 1
        needsPresent = false
        stats.submittedFrames &+= 1
        commandBuffer.present(drawable)
        commandBuffer.addCompletedHandler { [weak self] completed in
            let failed = completed.status == .error
            Task { @MainActor [weak self] in
                self?.commandCompleted(failed: failed)
            }
        }
        commandBuffer.commit()
        return true
    }

    /// Deterministic offscreen validation only. Production presentation never
    /// waits for GPU completion.
    func renderOffscreenAndWait(width: Int, height: Int) -> MTLTexture? {
        guard width > 0,
              height > 0,
              let instanceBuffer,
              instanceCount > 0,
              let atlasTexture = glyphAtlas.texture
        else {
            return nil
        }
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        descriptor.usage = [.renderTarget]
        descriptor.storageMode = .shared
        guard let texture = device.makeTexture(descriptor: descriptor),
              let commandBuffer = makeCommandBuffer(
                  target: texture,
                  instanceBuffer: instanceBuffer,
                  atlasTexture: atlasTexture
              )
        else {
            return nil
        }
        commandBuffer.commit()
        commandBuffer.waitUntilCompleted()
        return commandBuffer.status == .completed ? texture : nil
    }

    func handleDrawableUnavailable() {
        stats.drawableMisses &+= 1
        needsPresent = true
    }

    private func allocateInstanceBuffer(rows: Int, columns: Int) throws {
        let count = rows * columns
        let byteCount = count * MemoryLayout<TerminalInstance>.stride
        guard let buffer = device.makeBuffer(length: max(byteCount, 1), options: .storageModeShared) else {
            throw MetalTerminalRendererError.unavailableBuffer
        }
        buffer.label = "Seyal Terminal Instances"
        instanceBuffer = buffer
        instanceCount = count
        stats.instanceBufferAllocations &+= 1
        stats.instanceBytes = UInt64(byteCount)
    }

    private func rebuildRows(
        frame: NativePreparedFrame,
        damage: DamageMask,
        metrics: TerminalFontMetrics,
        backingScale: CGFloat
    ) throws {
        guard let instanceBuffer else {
            throw MetalTerminalRendererError.unavailableBuffer
        }
        let pointer = instanceBuffer.contents().bindMemory(
            to: TerminalInstance.self,
            capacity: instanceCount
        )
        let cellSize = SIMD2<Float>(Float(metrics.cellWidth), Float(metrics.cellHeight))

        for row in 0..<frame.rows where damage.contains(row: row) {
            for column in 0..<frame.columns {
                let index = row * frame.columns + column
                let cell = frame.cells[index]
                guard cell.reserved == 0 else {
                    throw MetalTerminalRendererError.invalidFrame
                }

                var flags: UInt32 = 0
                var uvRect = SIMD4<Float>(repeating: 0)
                var atlasSlice: UInt32 = 0
                if cell.scalar != 0 && cell.scalar != 32 {
                    let entry = try glyphAtlas.lookup(
                        scalar: cell.scalar,
                        bold: cell.flags & preparedBoldFlag != 0,
                        backingScale: backingScale,
                        cellMetrics: metrics
                    )
                    flags |= instanceGlyphFlag
                    uvRect = entry.uvRect
                    atlasSlice = entry.slice
                }
                if cell.flags & preparedUnderlineFlag != 0 {
                    flags |= instanceUnderlineFlag
                }
                if frame.cursorVisible,
                   row == frame.cursorRow,
                   column == frame.cursorColumn
                {
                    flags |= instanceCursorFlag
                }

                pointer[index] = TerminalInstance(
                    origin: SIMD2<Float>(
                        Float(column * metrics.cellWidth),
                        Float(row * metrics.cellHeight)
                    ),
                    size: cellSize,
                    uvRect: uvRect,
                    foreground: resolveTerminalColor(
                        cell.foreground,
                        defaultRGBA: 0xffe9_e1d8
                    ),
                    background: resolveTerminalColor(
                        cell.background,
                        defaultRGBA: 0xff10_0d0b
                    ),
                    flags: flags,
                    atlasSlice: atlasSlice
                )
            }
        }
    }

    private func makeCommandBuffer(
        target: MTLTexture,
        instanceBuffer: MTLBuffer,
        atlasTexture: MTLTexture
    ) -> MTLCommandBuffer? {
        guard let commandBuffer = commandQueue.makeCommandBuffer() else { return nil }
        commandBuffer.label = "Seyal Terminal Frame"
        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = target
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].storeAction = .store
        pass.colorAttachments[0].clearColor = MTLClearColor(
            red: 0.043,
            green: 0.051,
            blue: 0.063,
            alpha: 1
        )
        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: pass) else {
            return nil
        }
        encoder.label = "Seyal Terminal Encoder"
        encoder.setRenderPipelineState(pipeline)
        encoder.setVertexBuffer(instanceBuffer, offset: 0, index: 0)
        var viewport = SIMD2<Float>(Float(target.width), Float(target.height))
        encoder.setVertexBytes(
            &viewport,
            length: MemoryLayout<SIMD2<Float>>.stride,
            index: 1
        )
        encoder.setFragmentTexture(atlasTexture, index: 0)
        encoder.setFragmentSamplerState(sampler, index: 0)
        encoder.drawPrimitives(
            type: .triangle,
            vertexStart: 0,
            vertexCount: 6,
            instanceCount: instanceCount
        )
        encoder.endEncoding()
        return commandBuffer
    }

    private func commandCompleted(failed: Bool) {
        framesInFlight = 0
        stats.completedFrames &+= 1
        if failed {
            deferredNeedsFullRebuild = true
            needsCurrentFrameWhenIdle = true
        }
        if releaseWhenIdle || !visible {
            releaseWhenIdle = false
            releaseDedicatedResources()
            return
        }
        if !deferredDamage.isEmpty || deferredNeedsFullRebuild || needsCurrentFrameWhenIdle {
            requestCurrentFrameIfNeeded()
        }
    }

    private func requestCurrentFrameIfNeeded() {
        guard framesInFlight == 0 else {
            needsCurrentFrameWhenIdle = true
            return
        }
        needsCurrentFrameWhenIdle = false
        onNeedsCurrentFrame?()
    }

    private func releaseDedicatedResources() {
        instanceBuffer = nil
        instanceCount = 0
        currentRows = 0
        currentColumns = 0
        currentMetrics = nil
        currentScale = 0
        stats.instanceBytes = 0
        glyphAtlas.releaseResourcesWhenGPUIdle()
        deferredNeedsFullRebuild = true
    }
}

private func resolveTerminalColor(_ packed: UInt32, defaultRGBA: UInt32) -> UInt32 {
    let tag = packed & 0xff00_0000
    if tag == 0 {
        return defaultRGBA
    }
    if tag == 0x0200_0000 {
        return packRGBA(
            red: UInt8((packed >> 16) & 0xff),
            green: UInt8((packed >> 8) & 0xff),
            blue: UInt8(packed & 0xff)
        )
    }
    if tag == 0x0100_0000 {
        return indexedColor(UInt8(packed & 0xff))
    }
    return defaultRGBA
}

private func indexedColor(_ index: UInt8) -> UInt32 {
    let base: [(UInt8, UInt8, UInt8)] = [
        (0, 0, 0), (205, 49, 49), (13, 188, 121), (229, 229, 16),
        (36, 114, 200), (188, 63, 188), (17, 168, 205), (229, 229, 229),
        (102, 102, 102), (241, 76, 76), (35, 209, 139), (245, 245, 67),
        (59, 142, 234), (214, 112, 214), (41, 184, 219), (255, 255, 255),
    ]
    let value = Int(index)
    if value < 16 {
        let rgb = base[value]
        return packRGBA(red: rgb.0, green: rgb.1, blue: rgb.2)
    }
    if value < 232 {
        let cube = value - 16
        let red = cube / 36
        let green = (cube % 36) / 6
        let blue = cube % 6
        func component(_ value: Int) -> UInt8 {
            value == 0 ? 0 : UInt8(55 + value * 40)
        }
        return packRGBA(
            red: component(red),
            green: component(green),
            blue: component(blue)
        )
    }
    let gray = UInt8(8 + (value - 232) * 10)
    return packRGBA(red: gray, green: gray, blue: gray)
}

private func packRGBA(red: UInt8, green: UInt8, blue: UInt8) -> UInt32 {
    UInt32(red)
        | (UInt32(green) << 8)
        | (UInt32(blue) << 16)
        | 0xff00_0000
}
